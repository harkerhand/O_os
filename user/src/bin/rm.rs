#![no_std]
#![no_main]

extern crate alloc;

#[macro_use]
extern crate user_lib;

use alloc::{string::String, vec::Vec};
use user_lib::{OpenFlags, close, open, read, unlink};

const DIRENT_SIZE: usize = 32;
const DIRENT_NAME_SIZE: usize = 28;

fn to_cstr(path: &str) -> String {
    let mut s = String::from(path);
    s.push('\0');
    s
}

fn join_path(parent: &str, name: &str) -> String {
    if parent == "/" {
        let mut s = String::from("/");
        s.push_str(name);
        return s;
    }
    let mut s = String::from(parent);
    if !s.ends_with('/') {
        s.push('/');
    }
    s.push_str(name);
    s
}

fn read_dir_entries(path: &str) -> Result<Vec<String>, ()> {
    let c_path = to_cstr(path);
    let fd = open(&c_path, OpenFlags::RDONLY | OpenFlags::DIRECTORY);
    if fd < 0 {
        return Err(());
    }
    let fd = fd as usize;

    let mut entries = Vec::new();
    let mut buf = [0u8; DIRENT_SIZE * 16];
    loop {
        let n = read(fd, &mut buf);
        if n < 0 {
            close(fd);
            return Err(());
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
            {
                entries.push(String::from(name));
            }
            offset += DIRENT_SIZE;
        }
    }

    close(fd);
    Ok(entries)
}

fn rm_path(path: &str, recursive: bool) -> i32 {
    if path == "/" {
        println!("rm: 禁止删除根目录 /");
        return -1;
    }

    let c_path = to_cstr(path);
    let dir_fd = open(&c_path, OpenFlags::RDONLY | OpenFlags::DIRECTORY);
    if dir_fd >= 0 {
        close(dir_fd as usize);
        if !recursive {
            println!("rm: {} 是目录（请使用 -r）", path);
            return -1;
        }

        let entries = match read_dir_entries(path) {
            Ok(v) => v,
            Err(_) => {
                println!("rm: 读取目录失败 {}", path);
                return -1;
            }
        };
        let mut ret = 0;
        for name in entries {
            let child = join_path(path, &name);
            if rm_path(&child, true) != 0 {
                ret = -1;
            }
        }
        if unlink(&c_path) < 0 {
            println!("rm: 删除目录失败 {}", path);
            return -1;
        }
        return ret;
    }

    if unlink(&c_path) < 0 {
        println!("rm: 删除失败 {}", path);
        return -1;
    }
    0
}

#[unsafe(no_mangle)]
fn main(argc: usize, argv: &[&str]) -> i32 {
    if argc < 2 {
        println!("用法: rm [-r] <路径> [路径 ...]");
        return -1;
    }

    let mut recursive = false;
    let mut paths: Vec<&str> = Vec::new();
    for arg in argv.iter().skip(1) {
        if *arg == "-r" {
            recursive = true;
        } else {
            paths.push(*arg);
        }
    }
    if paths.is_empty() {
        println!("rm: 缺少路径参数");
        return -1;
    }

    let mut ret = 0;
    for path in paths {
        if rm_path(path, recursive) != 0 {
            ret = -1;
        }
    }
    ret
}
