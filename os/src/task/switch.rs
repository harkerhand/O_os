//! Rust wrapper around `__switch`.
//!
//! 切换任务上下文的 Rust 包装器。
use crate::task::TaskContext;
use core::arch::global_asm;

global_asm!(include_str!("switch.S"));

unsafe extern "C" {
    pub unsafe fn __switch(
        current_task_cx_ptr: *mut TaskContext,
        next_task_cx_ptr: *const TaskContext,
    );
}
