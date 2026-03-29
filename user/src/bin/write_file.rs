#![no_std]
#![no_main]

#[macro_use]
extern crate user_lib;
use user_lib::{OpenFlags, close, open, write};

#[unsafe(no_mangle)]
fn main(argc: usize, argv: &[&str]) -> i32 {
    if argc < 3 {
        println!("Usage: {} <filename> <text>", argv[0]);
        return -1;
    }

    let target_file = argv[1];
    let content = argv[2];

    // 打开文件：创建 | 只写
    let fd = open(target_file, OpenFlags::CREATE | OpenFlags::WRONLY);
    if fd < 0 {
        println!("Failed to open {}", target_file);
        return -1;
    }

    // 写入内容
    let write_size = write(fd as usize, content.as_bytes());
    if write_size < 0 {
        println!("Write failed!");
    } else {
        println!("Wrote {} bytes to {}", write_size, target_file);
    }

    close(fd as usize);
    0
}
