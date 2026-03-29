#![no_std]
#![no_main]

extern crate alloc;

#[macro_use]
extern crate user_lib;

use alloc::{string::String, vec::Vec};
use user_lib::{OpenFlags, close, open};

fn to_cstr(path: &str) -> String {
    let mut s = String::from(path);
    s.push('\0');
    s
}

fn mkdir_one(path: &str, parents: bool) -> i32 {
    let c_path = to_cstr(path);

    let fd = open(&c_path, OpenFlags::RDONLY | OpenFlags::DIRECTORY);
    if fd >= 0 {
        close(fd as usize);
        if parents {
            return 0;
        }
        println!("mkdir: 目录已存在 {}", path);
        return -1;
    }

    let fd = open(&c_path, OpenFlags::CREATE | OpenFlags::DIRECTORY);
    if fd < 0 {
        println!("mkdir: 无法创建目录 {}", path);
        return -1;
    }

    close(fd as usize);
    0
}

#[unsafe(no_mangle)]
fn main(argc: usize, argv: &[&str]) -> i32 {
    if argc < 2 {
        println!("用法: mkdir [-p] <目录> [目录 ...]");
        return -1;
    }

    let mut parents = false;
    let mut paths: Vec<&str> = Vec::new();
    for arg in argv.iter().skip(1) {
        if *arg == "-p" {
            parents = true;
        } else {
            paths.push(*arg);
        }
    }
    if paths.is_empty() {
        println!("mkdir: 缺少目录参数");
        return -1;
    }

    let mut ret = 0;
    for path in paths {
        if mkdir_one(path, parents) != 0 {
            ret = -1;
        }
    }
    ret
}
