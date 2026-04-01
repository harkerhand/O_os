#![no_std]
#![no_main]

#[macro_use]
extern crate user_lib;
extern crate alloc;

use log::info;
use user_lib::{exit, mutex_create, mutex_lock, mutex_unlock, sleep_ms, thread_create, waittid};

pub fn thread_a(mutex_id: usize) -> ! {
    mutex_lock(mutex_id);
    sleep_ms(2000);
    println!("thread a: got the lock, sleep for 2 seconds");
    mutex_unlock(mutex_id);
    exit(1)
}

pub fn thread_b(mutex_id: usize) -> ! {
    sleep_ms(100);
    mutex_lock(mutex_id);
    println!("thread b: got the lock");
    mutex_unlock(mutex_id);
    exit(2)
}

#[unsafe(no_mangle)]
pub fn main() -> i32 {
    let mutex_id = mutex_create();

    let v = [
        thread_create(thread_a as *const () as usize, mutex_id as usize),
        thread_create(thread_b as *const () as usize, mutex_id as usize),
    ];
    info!("main thread created threads: {:?}", v);
    for tid in v.iter() {
        let exit_code = waittid(*tid as usize);
        println!("thread#{} exited with code {}", tid, exit_code);
    }
    0
}
