#![no_std]
#![no_main]

#[macro_use]
extern crate user_lib;

use riscv::register::sstatus::{self, SPP};

#[unsafe(no_mangle)]
fn main() -> i32 {
    println!("尝试切换到 U 模式，看看内核能否正确处理这个非法指令异常");
    unsafe {
        sstatus::set_spp(SPP::User);
    }
    0
}
