#![no_std]
#![no_main]

extern crate alloc;
extern crate user_lib;
use alloc::boxed::Box;

use log::info;

#[unsafe(no_mangle)]
fn main() -> i32 {
    recursive(0);
    0
}

#[allow(unconditional_recursion)]
fn recursive(depth: usize) {
    let _buf = Box::new([0u8; 0x1000000]); // 占用 16MB 的堆空间
    core::hint::black_box(&_buf); // 防止编译器优化掉 _buf
    info!("depth: {}", depth);
    recursive(depth + 1);
}
