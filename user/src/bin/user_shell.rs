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
    if matches!(cmd, "ls" | "mkdir" | "cat" | "write_file") {
        let mut s = String::from("/");
        s.push_str(cmd);
        s
    } else {
        String::from(cmd)
    }
}

#[unsafe(no_mangle)]
pub fn main() -> i32 {
    let mut line: String = String::new();
    blue!("{} >> ", shell_prompt());
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
                        blue!("{} >> ", shell_prompt());
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
                }
                blue!("{} >> ", shell_prompt());
            }
            BS | DL => {
                if !line.is_empty() {
                    print!("{}", BS as char);
                    print!(" ");
                    print!("{}", BS as char);
                    line.pop();
                }
            }
            _ => {
                print!("{}", c as char);
                line.push(c as char);
            }
        }
    }
    0
}
