//! 进程相关的系统调用
use log::info;

use crate::task::{exit_current_and_run_next, suspend_current_and_run_next};
use crate::timer::get_time_ms;

/// 系统调用：退出当前应用并运行下一个应用
pub fn sys_exit(exit_code: i32) -> ! {
    info!("[kernel] 应用退出，退出码为 {}", exit_code);
    exit_current_and_run_next();
    panic!("Unreachable in sys_exit!");
}

/// 系统调用：让出 CPU 给其他应用
pub fn sys_yield() -> isize {
    suspend_current_and_run_next();
    0
}

/// 系统调用：获取当前时间（单位：毫秒）
pub fn sys_get_time() -> isize {
    get_time_ms() as isize
}
