#![no_std]
#![no_main]

#[macro_use]
extern crate user_lib;

use core::arch::asm;

#[unsafe(no_mangle)]
fn main() -> i32 {
    println!("尝试执行一个特权指令，看看内核能否正确处理这个非法指令异常");
    unsafe {
        asm!("sret");
    }
    0
}
