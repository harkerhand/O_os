#![no_std]
#![no_main]

#[macro_use]
extern crate user_lib;

#[unsafe(no_mangle)]
fn main() -> i32 {
    println!("尝试访问一个非法地址，看看内核能否正确处理这个缺页异常");
    unsafe {
        core::ptr::null_mut::<u8>().write_volatile(0);
    }
    0
}
