#![no_std]
#![feature(linkage)]
#![feature(alloc_error_handler)]

#[macro_use]
pub mod console;
mod heap_alloc;
mod lang_items;
mod logging;
mod syscall;

#[alloc_error_handler]
pub fn handle_alloc_error(layout: core::alloc::Layout) -> ! {
    error!("内存分配失败: {:?}", layout);
    exit(-1)
}

#[unsafe(no_mangle)]
#[unsafe(link_section = ".text.entry")]
pub extern "C" fn _start() -> ! {
    logging::init();
    exit(main());
}

#[linkage = "weak"]
#[unsafe(no_mangle)]
fn main() -> i32 {
    panic!("Cannot find main!");
}

use log::error;
use syscall::*;

pub fn getchar() -> u8 {
    let mut buf = [0u8; 1];
    sys_read(0, &mut buf);
    buf[0]
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

pub fn exec(path: &str) -> isize {
    sys_exec(path.as_ptr())
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
