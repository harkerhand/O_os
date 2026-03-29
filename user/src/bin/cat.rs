#![no_std]
#![no_main]

extern crate alloc;

#[macro_use]
extern crate user_lib;

use alloc::string::String;
use user_lib::{OpenFlags, close, open, read};

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
        println!("{}", core::str::from_utf8(&buf[..n]).unwrap());
    }

    close(fd);
    0
}

#[unsafe(no_mangle)]
fn main(argc: usize, argv: &[&str]) -> i32 {
    if argc < 2 {
        println!("用法: cat <文件> [文件 ...]");
        return -1;
    }

    let mut ret = 0;
    for path in argv.iter().skip(1) {
        if cat_one(path) != 0 {
            ret = -1;
        }
    }
    ret
}
