#![no_std]
#![no_main]

extern crate alloc;

#[macro_use]
extern crate user_lib;

use alloc::string::String;
use user_lib::{OpenFlags, close, getchar, open, read, write};

const CTRL_C: u8 = 3;
const LF: u8 = b'\n';
const CR: u8 = b'\r';

fn to_cstr(path: &str) -> String {
    let mut s = String::from(path);
    s.push('\0');
    s
}

fn cat_one(path: &str) -> i32 {
    let c_path = to_cstr(path);
    let fd = open(&c_path, OpenFlags::RDONLY);
    if fd < 0 {
        println!("cat: 无法打开文件 {}", path);
        return -1;
    }
    let fd = fd as usize;

    let mut buf = [0u8; 512];
    loop {
        let n = read(fd, &mut buf);
        if n < 0 {
            println!("cat: 读取文件失败 {}", path);
            close(fd);
            return -1;
        }
        if n == 0 {
            break;
        }
        let n = n as usize;
        if write(1, &buf[..n]) < 0 {
            println!("cat: 写入标准输出失败");
            close(fd);
            return -1;
        }
    }

    close(fd);
    0
}

#[unsafe(no_mangle)]
fn main(argc: usize, argv: &[&str]) -> i32 {
    if argc < 2 {
        let mut line = String::new();
        loop {
            let c = getchar();
            match c {
                0 | CTRL_C => {
                    print!("{}", line);
                    return 0;
                }
                LF | CR => {
                    println!("{}", line);
                    line.clear();
                }
                _ => {
                    line.push(c as char);
                }
            }
        }
    }

    let mut ret = 0;
    for path in argv.iter().skip(1) {
        if cat_one(path) != 0 {
            ret = -1;
        }
    }
    ret
}
