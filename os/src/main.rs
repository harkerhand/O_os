//! 内核的入口函数

#![deny(missing_docs)]
#![deny(warnings)]
#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

use crate::{fs::inode::list_apps, timer::sleep_ms};

extern crate alloc;

#[macro_use]
mod console;
mod config;
mod drivers;
mod error;
mod fs;
mod lang_items;
mod logging;
mod mem;
mod sbi;
mod sync;
mod syscall;
mod task;
mod timer;
mod trap;

core::arch::global_asm!(include_str!("entry.asm"));
core::arch::global_asm!(include_str!("link_app.S"));

/// 清空 BSS 段
pub fn clear_bss() {
    unsafe extern "C" {
        safe fn sbss();
        safe fn ebss();
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
    mem::init();
    print_logo();
    trap::init();
    trap::enable_timer_interrupt();
    timer::set_next_trigger();
    list_apps();
    task::run();
    panic!("Unreachable in rust_main!");
}

fn print_logo() {
    let time_ms = 200;
    for _ in 0..4 {
        println!("{}", include_str!("./logos/0.txt"));
        sleep_ms(time_ms);
        print!("\x1b[8A\r");
        println!("{}", include_str!("./logos/1.txt"));
        sleep_ms(time_ms);
        print!("\x1b[8A\r");
        println!("{}", include_str!("./logos/2.txt"));
        sleep_ms(time_ms);
        print!("\x1b[8A\r");
        println!("{}", include_str!("./logos/3.txt"));
        sleep_ms(time_ms);
        print!("\x1b[8A\r");
    }
    println!("{}", include_str!("./logos/0.txt"));
}
