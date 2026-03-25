//! 内核的入口函数

#![deny(missing_docs)]
#![deny(warnings)]
#![no_std]
#![no_main]

#[macro_use]
mod console;
mod batch;
mod lang_items;
mod logging;
mod sbi;
mod sync;
mod syscall;
mod trap;

core::arch::global_asm!(include_str!("entry.asm"));
core::arch::global_asm!(include_str!("link_app.S"));

/// 清空 BSS 段
pub fn clear_bss() {
    unsafe extern "C" {
        fn sbss();
        fn ebss();
    }

    let start = sbss as *const () as usize;
    let end = ebss as *const () as usize;
    for addr in start..end {
        unsafe { (addr as *mut u8).write_volatile(0) };
    }
}

/// O_os 的入口函数
#[unsafe(no_mangle)]
pub fn rust_main() -> ! {
    clear_bss();
    logging::init();
    trap::init();
    batch::init();
    batch::run_next_app();
}
