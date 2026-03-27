#![no_std]
#![no_main]

extern crate user_lib;

use core::ptr::{null_mut, read_volatile};

use log::info;

#[unsafe(no_mangle)]
fn main() -> i32 {
    info!("读取一个空指针，触发页错误...");
    unsafe {
        let _i = read_volatile(null_mut::<u8>());
    }
    0
}
