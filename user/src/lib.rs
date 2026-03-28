#![no_std]
#![feature(linkage)]
#![feature(alloc_error_handler)]

#[macro_use]
pub mod console;
mod heap_alloc;
mod lang_items;
mod logging;
mod sync;
mod syscall;

extern crate alloc;

#[alloc_error_handler]
pub fn handle_alloc_error(layout: core::alloc::Layout) -> ! {
    error!("内存分配失败: {:?}", layout);
    exit(-1)
}

#[unsafe(no_mangle)]
#[unsafe(link_section = ".text.entry")]
pub extern "C" fn _start(argc: usize, argv: usize) -> ! {
    logging::init();
    let mut v = Vec::new();
    for i in 0..argc {
        let str_start = unsafe {
            let argv_ptr = (argv + i * core::mem::size_of::<usize>()) as *const usize;
            argv_ptr.read_volatile()
        };
        let len = (0..)
            .find(|i| unsafe { ((str_start + i) as *const u8).read_volatile() == 0 })
            .unwrap();
        v.push(
            core::str::from_utf8(unsafe {
                core::slice::from_raw_parts(str_start as *const u8, len)
            })
            .unwrap(),
        );
    }
    debug!("应用入口，argc: {}, argv: {:?}", argc, v);

    exit(main(argc, v.as_slice()));
}

#[linkage = "weak"]
#[unsafe(no_mangle)]
fn main(_argc: usize, _argv: &[&str]) -> i32 {
    panic!("Cannot find main!");
}

use alloc::vec::Vec;
use log::{debug, error};
use syscall::*;

pub use console::getchar;

pub fn read(fs: usize, buf: &mut [u8]) -> isize {
    sys_read(fs, buf)
}

pub fn write(fd: usize, buf: &[u8]) -> isize {
    sys_write(fd, buf)
}
pub fn exit(exit_code: i32) -> ! {
    sys_exit(exit_code)
}
pub fn yield_() -> isize {
    sys_yield()
}
pub fn get_time() -> isize {
    sys_get_time()
}

pub fn sbrk(size: i32) -> isize {
    sys_sbrk(size)
}

pub fn mmap(addr: usize, length: usize, prot: usize) -> isize {
    sys_mmap(addr, length, prot)
}

pub fn munmap(addr: usize, length: usize) -> isize {
    sys_munmap(addr, length)
}

pub fn fork() -> isize {
    sys_fork()
}

pub fn exec(path: &str, args: &[*const u8]) -> isize {
    sys_exec(path, args)
}

pub fn waitpid(pid: isize, status: &mut i32) -> isize {
    match sys_waitpid(pid, status as *mut i32) {
        -2 => {
            sys_yield();
            waitpid(pid, status)
        }
        pid => pid,
    }
}

pub fn wait(exit_code: &mut i32) -> isize {
    loop {
        match sys_waitpid(-1, exit_code as *mut i32) {
            -2 => {
                sys_yield();
            }
            pid => return pid,
        }
    }
}

bitflags::bitflags! {
    pub struct OpenFlags: u32 {
        const RDONLY = 0;
        const WRONLY = 1 << 0;
        const RDWR = 1 << 1;
        const CREATE = 1 << 9;
        const TRUNC = 1 << 10;
        const DIRECTORY = 1 << 11;
    }
}

pub fn open(path: &str, flags: OpenFlags) -> isize {
    sys_openat(path, flags.bits())
}

pub fn close(fd: usize) -> isize {
    sys_close(fd)
}
