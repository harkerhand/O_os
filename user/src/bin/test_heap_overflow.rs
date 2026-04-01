#![no_std]
#![no_main]

extern crate alloc;
extern crate user_lib;
use alloc::boxed::Box;

use log::info;

#[unsafe(no_mangle)]
fn main() -> i32 {
    let mut v = alloc::vec::Vec::new();
    let mut count = 0;

    info!("开始分配小碎片测试...");
    loop {
        // 尝试分配 1KB
        let box_data = Box::new([0u8; 1024]);

        // 关键：将 Box 丢进 Vec，确保内存不被释放
        v.push(box_data);

        count += 1;
        if count % 100 == 0 {
            info!("已成功分配 {} 个碎片 ({} KB)", count, count);
        }
    }
}
