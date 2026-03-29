#![no_std]
#![no_main]

extern crate alloc;

#[macro_use]
extern crate user_lib;

use alloc::string::String;
use user_lib::{OpenFlags, close, open};

fn to_cstr(path: &str) -> String {
    let mut s = String::from(path);
    s.push('\0');
    s
}

fn touch_one(path: &str) -> i32 {
    let c_path = to_cstr(path);

    let fd = open(&c_path, OpenFlags::RDONLY);
    if fd >= 0 {
        close(fd as usize);
        return 0;
    }

    let fd = open(&c_path, OpenFlags::CREATE | OpenFlags::WRONLY);
    if fd < 0 {
        println!("touch: 无法创建文件 {}", path);
        return -1;
    }

    close(fd as usize);
    0
}

#[unsafe(no_mangle)]
fn main(argc: usize, argv: &[&str]) -> i32 {
    if argc < 2 {
        println!("用法: touch <文件> [文件 ...]");
        return -1;
    }

    let mut ret = 0;
    for path in argv.iter().skip(1) {
        if touch_one(path) != 0 {
            ret = -1;
        }
    }
    ret
}
