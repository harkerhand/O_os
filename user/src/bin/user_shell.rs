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
use user_lib::{exec, fork, getchar, waitpid};

#[unsafe(no_mangle)]
pub fn main() -> i32 {
    println!("Rust user shell");
    let mut line: String = String::new();
    print!(">> ");
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
                    let args: Vec<_> = line.split_whitespace().collect();
                    let args_copy: Vec<String> = args
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
                print!(">> ");
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
