#![no_std]
#![no_main]

extern crate user_lib;

use core::ptr::null_mut;

use log::info;

#[unsafe(no_mangle)]
fn main() -> i32 {
    info!("写入一个空指针，触发页错误...");
    unsafe {
        null_mut::<u8>().write_volatile(1);
    }
    0
}
