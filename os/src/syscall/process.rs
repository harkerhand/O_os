//! 进程相关的系统调用
use log::info;

use crate::batch::run_next_app;

/// 系统调用：退出当前应用并运行下一个应用
pub fn sys_exit(exit_code: i32) -> ! {
    info!("[kernel] 应用退出，退出码为 {}", exit_code);
    run_next_app()
}
