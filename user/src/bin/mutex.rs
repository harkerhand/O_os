#![no_std]
#![no_main]

#[macro_use]
extern crate user_lib;
extern crate alloc;

use alloc::sync::Arc;
use log::info;
use user_lib::{exit, sleep_ms, sync::Mutex, thread_create, waittid};

pub fn thread_a(mutex: &Arc<Mutex<i32>>) -> ! {
    let guard = mutex.lock();
    println!("thread a: got the lock, sleep for 2 seconds");
    sleep_ms(2000);
    println!("thread a: done sleeping, release the lock and exit");
    drop(guard);
    exit(1)
}

pub fn thread_b(mutex: &Arc<Mutex<i32>>) -> ! {
    sleep_ms(100);
    let guard = mutex.lock();
    println!("thread b: got the lock");
    drop(guard);
    exit(2)
}

#[unsafe(no_mangle)]
pub fn main() -> i32 {
    let a = Arc::new(Mutex::new(0));
    info!("arc created: {:?}", a);

    let v = [
        thread_create(thread_a as *const () as usize, &a as *const _ as usize),
        thread_create(thread_b as *const () as usize, &a as *const _ as usize),
    ];
    info!("main thread created threads: {:?}", v);
    for tid in v.iter() {
        let exit_code = waittid(*tid as usize);
        println!("thread#{} exited with code {}", tid, exit_code);
    }
    0
}
