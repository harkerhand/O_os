//! syscalls 的实现
//! 当用户空间使用 `ecall` 指令发起系统调用时，
//! 处理器会引发一个 '来自 U 模式的环境调用' 异常，
//! 这在 [`crate::trap::trap_handler`] 的某个 case 中被处理。
//! 为了清晰起见，每个系统调用都被实现为一个独立的函数，
//! 命名为 `sys_` 加上系统调用的名称。
//! 你可以在子模块中找到这样的函数，你也应该以这种方式实现系统调用。

const SYSCALL_WRITE: usize = 64;
const SYSCALL_EXIT: usize = 93;
const SYSCALL_YIELD: usize = 124;
const SYSCALL_GET_TIME: usize = 169;

mod fs;
mod process;

use fs::*;
use process::*;

/// 系统调用的入口函数
pub fn syscall(syscall_id: usize, args: [usize; 3]) -> isize {
    match syscall_id {
        SYSCALL_WRITE => sys_write(args[0], args[1] as *const u8, args[2]),
        SYSCALL_EXIT => sys_exit(args[0] as i32),
        SYSCALL_YIELD => sys_yield(),
        SYSCALL_GET_TIME => sys_get_time(),
        _ => panic!("Unsupported syscall_id: {}", syscall_id),
    }
}
