#![no_std]
#![no_main]

extern crate alloc;

#[macro_use]
extern crate user_lib;

use alloc::string::String;
use user_lib::unlink;

fn to_cstr(path: &str) -> String {
    let mut s = String::from(path);
    s.push('\0');
    s
}

fn rm_one(path: &str) -> i32 {
    let c_path = to_cstr(path);
    if unlink(&c_path) < 0 {
        println!("rm: 删除失败 {}", path);
        return -1;
    }
    0
}

#[unsafe(no_mangle)]
fn main(argc: usize, argv: &[&str]) -> i32 {
    if argc < 2 {
        println!("用法: rm <文件> [文件 ...]");
        return -1;
    }

    let mut ret = 0;
    for path in argv.iter().skip(1) {
        if rm_one(path) != 0 {
            ret = -1;
        }
    }
    ret
}
