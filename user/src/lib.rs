#![no_std]
#![feature(linkage)]
#![feature(alloc_error_handler)]

#[macro_use]
pub mod console;
mod heap_alloc;
mod lang_items;
mod logging;
pub mod sync;
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

use alloc::string::String;
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

pub fn unlink(path: &str) -> isize {
    sys_unlinkat(path)
}

pub fn chdir(path: &str) -> isize {
    sys_chdir(path)
}

pub fn getcwd(buf: &mut [u8]) -> isize {
    sys_getcwd(buf)
}

pub fn getcwd_string() -> Option<String> {
    let mut buf = [0u8; 256];
    let n = getcwd(&mut buf);
    if n <= 0 {
        return None;
    }
    let n = n as usize;
    if n == 0 || n > buf.len() {
        return None;
    }
    let str_len = n.saturating_sub(1);
    core::str::from_utf8(&buf[..str_len]).ok().map(String::from)
}

pub fn close(fd: usize) -> isize {
    sys_close(fd)
}

pub fn pipe(fd: &mut [usize; 2]) -> isize {
    sys_pipe(fd.as_mut_ptr())
}

pub fn dup(fd: usize) -> isize {
    sys_dup(fd)
}

pub fn thread_create(entry: usize, arg: usize) -> isize {
    sys_thread_create(entry, arg)
}
pub fn gettid() -> isize {
    sys_gettid()
}
pub fn waittid(tid: usize) -> isize {
    loop {
        match sys_waittid(tid) {
            -2 => {
                yield_();
            }
            exit_code => return exit_code,
        }
    }
}

pub fn mutex_create() -> isize {
    sys_mutex_create(false)
}
pub fn mutex_blocking_create() -> isize {
    sys_mutex_create(true)
}
pub fn mutex_lock(mutex_id: usize) {
    sys_mutex_lock(mutex_id);
}
pub fn mutex_unlock(mutex_id: usize) {
    sys_mutex_unlock(mutex_id);
}
pub fn semaphore_create(res_count: usize) -> isize {
    sys_semaphore_create(res_count)
}
pub fn semaphore_up(sem_id: usize) {
    sys_semaphore_up(sem_id);
}
pub fn semaphore_down(sem_id: usize) {
    sys_semaphore_down(sem_id);
}
pub fn condvar_create() -> isize {
    sys_condvar_create()
}
pub fn condvar_signal(condvar_id: usize) {
    sys_condvar_signal(condvar_id);
}
pub fn condvar_wait(condvar_id: usize, mutex_id: usize) {
    sys_condvar_wait(condvar_id, mutex_id);
}

pub fn sleep_ms(ms: usize) {
    sys_sleep(ms);
}
