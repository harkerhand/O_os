#![no_std]
#![no_main]

extern crate user_lib;

use log::{debug, info};
use user_lib::{get_time, yield_};

#[unsafe(no_mangle)]
fn main() -> i32 {
    let current_timer = get_time();
    let wait_for = current_timer + 500;
    while get_time() < wait_for {
        debug!(
            "sleeping... current_timer: {}, wait_for: {}",
            get_time(),
            wait_for
        );
        yield_();
    }
    info!("Test sleep OK!");
    0
}
