#![no_std]
#![no_main]

use user_lib::{SignalFlags, kill};

extern crate alloc;

#[macro_use]
extern crate user_lib;

#[unsafe(no_mangle)]
fn main(argc: usize, argv: &[&str]) -> i32 {
    if argc < 2 {
        println!("用法: kill <PID> [信号]");
        return -1;
    }
    let pid = argv[1].parse::<usize>().unwrap_or(0);
    let signal = if argc > 2 {
        if let Some(signal) = SignalFlags::from_bits(argv[2].parse::<u32>().unwrap_or(0)) {
            signal
        } else {
            println!("无效的信号: {}", argv[2]);
            return -1;
        }
    } else {
        SignalFlags::SIGABRT
    };
    kill(pid, signal);
    0
}
