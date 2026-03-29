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
use user_lib::{chdir, exec, fork, getchar, getcwd_string, waitpid};

fn shell_prompt() -> String {
    getcwd_string().unwrap_or_else(|| String::from("/"))
}

fn resolve_exec_path(cmd: &str) -> String {
    if matches!(cmd, "ls" | "mkdir" | "cat" | "touch" | "write_file") {
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
                    let raw_args: Vec<_> = line.split_whitespace().collect();
                    if raw_args[0] == "cd" {
                        if raw_args.len() != 2 {
                            println!("用法: cd <目录>");
                        } else {
                            let mut path = String::from(raw_args[1]);
                            path.push('\0');
                            if chdir(path.as_str()) < 0 {
                                println!("cd: 不存在的目录 {}", raw_args[1]);
                            }
                        }
                        line.clear();
                        green!("{} > ", shell_prompt());
                        continue;
                    }

                    let args_copy: Vec<String> = raw_args
                        .iter()
                        .map(|&arg| {
                            let mut string = arg.to_string();
                            string.push('\0');
                            string
                        })
                        .collect();
                    let mut args_addr: Vec<*const u8> =
                        args_copy.iter().map(|arg| arg.as_ptr()).collect();
                    args_addr.push(core::ptr::null());

                    let mut exec_path = resolve_exec_path(raw_args[0]);
                    exec_path.push('\0');
                    let pid = fork();
                    if pid == 0 {
                        // child process
                        if exec(exec_path.as_str(), &args_addr) == -1 {
                            println!("Error when executing!");
                            return -4;
                        }
                        unreachable!();
                    } else {
                        let mut exit_code: i32 = 0;
                        let exit_pid = waitpid(pid, &mut exit_code);
                        assert_eq!(pid, exit_pid);
                        info!("Shell: Process {} exited with code {}", pid, exit_code);
                    }
                    line.clear();
                    cursor = 0;
                }
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
