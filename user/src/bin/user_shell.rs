#![no_std]
#![no_main]
#![allow(clippy::println_empty_string)]

extern crate alloc;

#[macro_use]
extern crate user_lib;

const LF: u8 = 0x0au8;
const CR: u8 = 0x0du8;
const DL: u8 = 0x7fu8;
const BS: u8 = 0x08u8;
const ESC: u8 = 0x1bu8;

use alloc::{
    string::{String, ToString},
    vec::Vec,
};
use log::info;
use user_lib::{
    OpenFlags, chdir, close, dup, exec, fork, getchar, getcwd_string, open, pipe, waitpid,
};

fn shell_prompt() -> String {
    getcwd_string().unwrap_or_else(|| String::from("/"))
}

fn resolve_exec_path(cmd: &str) -> String {
    if matches!(
        cmd,
        "ls\0" | "mkdir\0" | "cat\0" | "touch\0" | "write_file\0" | "rm\0"
    ) {
        let mut s = String::from("/");
        s.push_str(cmd);
        s
    } else {
        String::from(cmd)
    }
}

fn move_cursor_left() {
    print!("{}", BS as char);
}

fn move_cursor_right(ch: u8) {
    print!("{}", ch as char);
}

fn handle_escape_sequence(line: &String, cursor: &mut usize) {
    let next = getchar();
    if next != b'[' {
        return;
    }
    let ch = getchar();
    match ch {
        b'D' => {
            if *cursor > 0 {
                move_cursor_left();
                *cursor -= 1;
            }
        }
        b'C' => {
            if *cursor < line.len() {
                move_cursor_right(line.as_bytes()[*cursor]);
                *cursor += 1;
            }
        }
        b'A' | b'B' => {
            // 上下箭头暂不支持历史，直接忽略
        }
        _ => {
            // 吞掉诸如 ESC [ 3 ~ 这类扩展序列，避免污染输入行
            if !ch.is_ascii_alphabetic() && ch != b'~' {
                loop {
                    let tail = getchar();
                    if tail.is_ascii_alphabetic() || tail == b'~' {
                        break;
                    }
                }
            }
        }
    }
}

#[unsafe(no_mangle)]
pub fn main() -> i32 {
    let mut line: String = String::new();
    let mut cursor: usize = 0;
    green!("{} > ", shell_prompt());
    loop {
        let c = getchar();
        match c {
            LF | CR => {
                println!("");
                if line == "exit" {
                    println!("Bye!");
                    break;
                }
                if !line.is_empty() {
                    // 检测后台运行标志 (&)
                    let (command_line, run_background) = if line.trim_end() == "&" {
                        println!("错误: & 符号必须在命令后面");
                        line.clear();
                        cursor = 0;
                        green!("{} > ", shell_prompt());
                        continue;
                    } else if line.ends_with(" &") || line.ends_with("\t&") {
                        (
                            line[..line.rfind('&').unwrap()].trim_end().to_string(),
                            true,
                        )
                    } else {
                        (line.clone(), false)
                    };

                    let process_arguments_list: Vec<_> = command_line
                        .as_str()
                        .split('|')
                        .map(ProcessArguments::new)
                        .collect();
                    let mut valid = true;
                    for (i, process_args) in process_arguments_list.iter().enumerate() {
                        if i == 0 {
                            if !process_args.output.is_empty() {
                                valid = false;
                            }
                        } else if i == process_arguments_list.len() - 1 {
                            if !process_args.input.is_empty() {
                                valid = false;
                            }
                        } else if !process_args.output.is_empty() || !process_args.input.is_empty()
                        {
                            valid = false;
                        }
                    }
                    if process_arguments_list.len() == 1 {
                        valid = true;
                        if process_arguments_list[0].args_copy[0] == "cd\0" {
                            if process_arguments_list[0].args_copy.len() != 2 {
                                println!("用法: cd <目录>");
                            } else if chdir(&process_arguments_list[0].args_copy[1]) < 0 {
                                println!(
                                    "cd: 不存在的目录 {}",
                                    process_arguments_list[0].args_copy[1]
                                );
                            }
                            line.clear();
                            cursor = 0;
                            green!("{} > ", shell_prompt());
                            continue;
                        }
                    }
                    if !valid {
                        println!("不支持的命令格式");
                        line.clear();
                        cursor = 0;
                        green!("{} > ", shell_prompt());
                        continue;
                    } else {
                        let mut pipes_fd = Vec::new();
                        if !process_arguments_list.is_empty() {
                            for _ in 0..process_arguments_list.len() - 1 {
                                let mut pipe_fd = [0usize; 2];
                                pipe(&mut pipe_fd);
                                pipes_fd.push(pipe_fd);
                            }
                        }
                        let mut children = Vec::new();
                        for (i, process_argument) in process_arguments_list.iter().enumerate() {
                            let pid = fork();
                            if pid == 0 {
                                let input = &process_argument.input;
                                let output = &process_argument.output;
                                let args_copy = &process_argument.args_copy;
                                let args_addr = &process_argument.args_addr;
                                // 重定向输入
                                if !input.is_empty() {
                                    let input_fd = open(input.as_str(), OpenFlags::RDONLY);
                                    if input_fd == -1 {
                                        println!("打开文件 {} 失败", input);
                                        return -4;
                                    }
                                    let input_fd = input_fd as usize;
                                    close(0);
                                    assert_eq!(dup(input_fd), 0);
                                    close(input_fd);
                                }
                                // 重定向输出
                                if !output.is_empty() {
                                    let output_fd = open(
                                        output.as_str(),
                                        OpenFlags::CREATE | OpenFlags::WRONLY,
                                    );
                                    if output_fd == -1 {
                                        println!("打开文件 {} 失败", output);
                                        return -4;
                                    }
                                    let output_fd = output_fd as usize;
                                    close(1);
                                    assert_eq!(dup(output_fd), 1);
                                    close(output_fd);
                                }
                                // 如果不是第一个进程，从上一个进程的管道读取输入
                                if i > 0 {
                                    close(0);
                                    let read_end = pipes_fd.get(i - 1).unwrap()[0];
                                    assert_eq!(dup(read_end), 0);
                                }
                                // 将输出发送到下一个进程的管道
                                if i < process_arguments_list.len() - 1 {
                                    close(1);
                                    let write_end = pipes_fd.get(i).unwrap()[1];
                                    assert_eq!(dup(write_end), 1);
                                }
                                // 子进程不需要管道的文件描述符，关闭它们以免泄漏
                                for pipe_fd in pipes_fd.iter() {
                                    close(pipe_fd[0]);
                                    close(pipe_fd[1]);
                                }
                                // 执行命令
                                let exec_path = resolve_exec_path(args_copy[0].as_str());
                                if exec(&exec_path, args_addr.as_slice()) == -1 {
                                    println!("执行命令时出错!");
                                    return -4;
                                }
                                unreachable!();
                            } else {
                                children.push(pid);
                            }
                        }
                        for pipe_fd in pipes_fd.iter() {
                            close(pipe_fd[0]);
                            close(pipe_fd[1]);
                        }

                        // 如果是后台运行，不等待子进程完成
                        if run_background {
                            println!("[后台进程启动] PID: {:?}", children);
                        } else {
                            // 前台运行：等待所有子进程完成
                            let mut exit_code: i32 = 0;
                            for pid in children.into_iter() {
                                let exit_pid = waitpid(pid, &mut exit_code);
                                assert_eq!(pid, exit_pid);
                                info!("Shell: 进程 {} 退出，代码 {}", pid, exit_code);
                            }
                        }
                    }
                }
                line.clear();
                cursor = 0;
                green!("{} > ", shell_prompt());
            }
            BS | DL => {
                if cursor > 0 {
                    let remove_idx = cursor - 1;
                    line.remove(remove_idx);
                    cursor -= 1;

                    move_cursor_left();
                    let tail = &line[remove_idx..];
                    print!("{}", tail);
                    print!(" ");
                    for _ in 0..(tail.len() + 1) {
                        move_cursor_left();
                    }
                }
            }
            ESC => {
                handle_escape_sequence(&line, &mut cursor);
            }
            _ => {
                // 仅接受可打印字符，避免控制字符污染命令行状态
                if c == b' ' || c.is_ascii_graphic() {
                    if cursor == line.len() {
                        print!("{}", c as char);
                        line.push(c as char);
                        cursor += 1;
                    } else {
                        line.insert(cursor, c as char);
                        let tail = &line[cursor..];
                        print!("{}", tail);
                        cursor += 1;
                        for _ in 0..(tail.len() - 1) {
                            move_cursor_left();
                        }
                    }
                }
            }
        }
    }
    0
}

#[derive(Debug)]
struct ProcessArguments {
    input: String,
    output: String,
    args_copy: Vec<String>,
    args_addr: Vec<*const u8>,
}

impl ProcessArguments {
    pub fn new(command: &str) -> Self {
        let args: Vec<_> = command.split(' ').collect();
        let mut args_copy: Vec<String> = args
            .iter()
            .filter(|&arg| !arg.is_empty())
            .map(|&arg| {
                let mut string = String::new();
                string.push_str(arg);
                string.push('\0');
                string
            })
            .collect();

        // redirect input
        let mut input = String::new();
        if let Some((idx, _)) = args_copy
            .iter()
            .enumerate()
            .find(|(_, arg)| arg.as_str() == "<\0")
        {
            input = args_copy[idx + 1].clone();
            args_copy.drain(idx..=idx + 1);
        }

        // redirect output
        let mut output = String::new();
        if let Some((idx, _)) = args_copy
            .iter()
            .enumerate()
            .find(|(_, arg)| arg.as_str() == ">\0")
        {
            output = args_copy[idx + 1].clone();
            args_copy.drain(idx..=idx + 1);
        }

        let mut args_addr: Vec<*const u8> = args_copy.iter().map(|arg| arg.as_ptr()).collect();
        args_addr.push(core::ptr::null::<u8>());

        Self {
            input,
            output,
            args_copy,
            args_addr,
        }
    }
}
