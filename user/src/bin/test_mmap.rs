#![no_std]
#![no_main]

extern crate user_lib;

use core::ptr::read_volatile;

use log::info;
use user_lib::{mmap, munmap};

#[unsafe(no_mangle)]
fn main() -> i32 {
    info!("Test mmap and munmap start.");
    const PAGE_SIZE: usize = 4096;
    let addr = 0x8000_0000;
    let ret = mmap(addr, PAGE_SIZE, 0x7); // 0x7 corresponds to PROT_READ | PROT_WRITE | PROT_EXEC
    assert_eq!(ret, 0);
    info!("尝试在已经映射的地址上再次调用 mmap，应该失败。");
    let ret = mmap(addr, PAGE_SIZE, 0x7); // 0x7 corresponds to PROT_READ | PROT_WRITE | PROT_EXEC
    assert_eq!(ret, -1);
    let unknown_num = addr + 0x100; // 在映射区域内的一个地址
    unsafe {
        let tmp: u32 = read_volatile(unknown_num as *const u32);
        info!(
            "成功访问了 mmap 映射的内存区域，说明 mmap 工作正常。值为: {}",
            tmp
        );
    }
    info!("多申请一些，然后统一回收。");
    let ret = mmap(addr + PAGE_SIZE, 2 * PAGE_SIZE, 0x7);
    assert_eq!(ret, 0);
    let ret = mmap(addr + 3 * PAGE_SIZE, 3 * PAGE_SIZE, 0x7);
    assert_eq!(ret, 0);
    let ret = munmap(addr, 2 * PAGE_SIZE);
    assert_eq!(ret, 0);
    let ret = munmap(addr + 2 * PAGE_SIZE, 2 * PAGE_SIZE);
    assert_eq!(ret, 0);
    let ret = munmap(addr + 4 * PAGE_SIZE, 2 * PAGE_SIZE);
    assert_eq!(ret, 0);
    info!("Test mmap and munmap passed.");

    0
}
