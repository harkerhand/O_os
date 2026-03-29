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
use user_lib::{OpenFlags, close, exec, fork, getchar, open, waitpid};

fn normalize_path(cwd: &str, input: &str) -> String {
    let mut parts: Vec<&str> = if input.starts_with('/') {
        Vec::new()
    } else {
        cwd.split('/').filter(|p| !p.is_empty()).collect()
    };
    for part in input.split('/') {
        if part.is_empty() || part == "." {
            continue;
        }
        if part == ".." {
            parts.pop();
        } else {
            parts.push(part);
        }
    }
    if parts.is_empty() {
        String::from("/")
    } else {
        let mut s = String::from("/");
        for (idx, part) in parts.iter().enumerate() {
            if idx > 0 {
                s.push('/');
            }
            s.push_str(part);
        }
        s
    }
}

fn check_dir_exists(path: &str) -> bool {
    let mut c_path = String::from(path);
    c_path.push('\0');
    let fd = open(&c_path, OpenFlags::RDONLY | OpenFlags::DIRECTORY);
    if fd < 0 {
        return false;
    }
    close(fd as usize);
    true
}

fn rewrite_args_with_cwd(raw_args: &[&str], cwd: &str) -> Vec<String> {
    if raw_args.is_empty() {
        return Vec::new();
    }
    let cmd = raw_args[0];
    let should_rewrite = matches!(cmd, "ls" | "cat" | "touch" | "rm" | "mkdir");
    let mut out = Vec::with_capacity(raw_args.len());
    out.push(String::from(cmd));
    for arg in raw_args.iter().skip(1) {
        if should_rewrite && !arg.starts_with('-') {
            out.push(normalize_path(cwd, arg));
        } else {
            out.push(String::from(*arg));
        }
    }
    out
}

#[unsafe(no_mangle)]
pub fn main() -> i32 {
    println!("Rust user shell");
    let mut line: String = String::new();
    let mut cwd = String::from("/");
    print!("{} >> ", cwd);
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
                            let new_cwd = normalize_path(&cwd, raw_args[1]);
                            if check_dir_exists(&new_cwd) {
                                cwd = new_cwd;
                            } else {
                                println!("cd: 不存在的目录 {}", raw_args[1]);
                            }
                        }
                        line.clear();
                        print!("{} >> ", cwd);
                        continue;
                    }

                    let argv = rewrite_args_with_cwd(&raw_args, &cwd);
                    let args_copy: Vec<String> = argv
                        .iter()
                        .map(|arg| {
                            let mut string = arg.to_string();
                            string.push('\0');
                            string
                        })
                        .collect();
                    let mut args_addr: Vec<*const u8> =
                        args_copy.iter().map(|arg| arg.as_ptr()).collect();
                    args_addr.push(core::ptr::null());
                    let pid = fork();
                    if pid == 0 {
                        // child process
                        if exec(args_copy[0].as_str(), &args_addr) == -1 {
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
                print!("{} >> ", cwd);
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
