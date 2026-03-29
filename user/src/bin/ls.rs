#![no_std]
#![no_main]

extern crate alloc;

#[macro_use]
extern crate user_lib;

use alloc::{
    string::{String, ToString},
    vec::Vec,
};
use user_lib::{OpenFlags, close, open, read};

const DIRENT_SIZE: usize = 32;
const DIRENT_NAME_SIZE: usize = 28;

fn to_cstr(path: &str) -> String {
    let mut s = String::from(path);
    s.push('\0');
    s
}

fn is_dir(path: &str) -> bool {
    let c_path = to_cstr(path);
    let fd = open(&c_path, OpenFlags::RDONLY | OpenFlags::DIRECTORY);
    if fd < 0 {
        return false;
    }
    close(fd as usize);
    true
}

#[unsafe(no_mangle)]
fn main(argc: usize, argv: &[&str]) -> i32 {
    let path = if argc > 1 { argv[1] } else { "." };
    let c_path = to_cstr(path);
    let fd = open(&c_path, OpenFlags::RDONLY | OpenFlags::DIRECTORY);
    if fd < 0 {
        println!("ls: 无法打开目录 {}", path);
        return -1;
    }
    let fd = fd as usize;

    let mut entries = Vec::new();
    let mut buf = [0u8; DIRENT_SIZE * 16];
    loop {
        let n = read(fd, &mut buf);
        if n < 0 {
            println!("ls: 读取目录失败 {}", path);
            close(fd);
            return -1;
        }
        if n == 0 {
            break;
        }
        let n = n as usize;
        let mut offset = 0;
        while offset + DIRENT_SIZE <= n {
            let entry = &buf[offset..offset + DIRENT_SIZE];
            let name_end = entry[..DIRENT_NAME_SIZE]
                .iter()
                .position(|&b| b == 0)
                .unwrap_or(DIRENT_NAME_SIZE);
            if name_end > 0
                && let Ok(name) = core::str::from_utf8(&entry[..name_end])
                && name != "initproc"
            {
                entries.push(name.to_string());
            }
            offset += DIRENT_SIZE;
        }
    }
    entries.sort();
    for (id, entry) in entries.iter().enumerate() {
        if is_dir(entry) {
            blue!("{}/", entry);
        } else {
            print!("{}", entry);
        }
        if id % 9 == 8 || id == entries.len() - 1 {
            println!("");
        } else {
            print!("  ");
        }
    }

    close(fd);
    0
}
