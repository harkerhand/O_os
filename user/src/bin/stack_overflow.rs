#![no_std]
#![no_main]

extern crate user_lib;

use log::info;

#[unsafe(no_mangle)]
fn main() -> i32 {
    recursive(0);
    0
}

#[allow(unconditional_recursion)]
fn recursive(depth: usize) {
    let _buf = [0u8; 1024]; // 占用 1K
    core::hint::black_box(&_buf); // 防止编译器优化掉 _buf
    info!("depth: {}", depth);
    recursive(depth + 1);
}
