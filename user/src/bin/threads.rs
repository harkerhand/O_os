#![no_std]
#![no_main]

#[macro_use]
extern crate user_lib;
extern crate alloc;

use core::hint::black_box;

use log::info;
use user_lib::{exit, thread_create, waittid};

pub fn thread_a() -> ! {
    for i in 0..100000 {
        let i = black_box(i);
        if i % 10000 == 0 {
            println!("thread a: {}", i);
        }
    }
    exit(1)
}

pub fn thread_b() -> ! {
    for i in 0..100000 {
        let i = black_box(i);
        if i % 10000 == 0 {
            println!("thread b: {}", i);
        }
    }
    exit(2)
}

pub fn thread_c() -> ! {
    for i in 0..100000 {
        let i = black_box(i);
        if i % 10000 == 0 {
            println!("thread c: {}", i);
        }
    }
    exit(3)
}

#[unsafe(no_mangle)]
pub fn main() -> i32 {
    let v = [
        thread_create(thread_a as *const () as usize, 0),
        thread_create(thread_b as *const () as usize, 0),
        thread_create(thread_c as *const () as usize, 0),
    ];
    info!("main thread created threads: {:?}", v);
    for tid in v.iter() {
        let exit_code = waittid(*tid as usize);
        println!("thread#{} exited with code {}", tid, exit_code);
    }
    println!("main thread exited.");
    0
}
