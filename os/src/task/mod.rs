//! 任务管理模块，负责管理和调度用户态的任务

mod context;
mod switch;

mod pid;
#[allow(clippy::module_inception)]
mod task;

use crate::fs::inode::{OpenFlags, open_file};
use crate::task::manager::{add_stopping_task, remove_from_pid2process};
use crate::task::pid::IDLE_PID;
use crate::task::proc::take_current_task;
use crate::task::task::ProcessControlBlock;
use crate::timer::remove_timer;
use alloc::sync::Arc;
use alloc::vec::Vec;
use log::info;
use task::TaskStatus;
mod manager;
mod proc;

pub use context::TaskContext;
pub use manager::*;
pub use proc::*;
pub use task::*;

/// 挂起当前任务，然后运行下一个任务
pub fn suspend_current_and_run_next() {
    let task = take_current_task();

    let mut task_inner = task.inner_exclusive_access();
    let task_cx_ptr = &mut task_inner.task_cx as *mut TaskContext;
    task_inner.task_status = TaskStatus::Ready;
    drop(task_inner);

    add_task(task);
    proc::schedule(task_cx_ptr);
}

/// 退出当前任务，然后运行下一个任务
pub fn exit_current_and_run_next(exit_code: i32) {
    let thread = take_current_task();
    let mut thread_inner = thread.inner_exclusive_access();
    let process = thread.process.upgrade().unwrap();
    let tid = thread_inner.res.as_ref().unwrap().tid;
    thread_inner.exit_code = Some(exit_code);
    // 非主线程的资源延迟到 waittid 回收，避免 tid 被过早复用。
    if tid == 0 {
        thread_inner.res = None;
    }
    drop(thread_inner);
    if tid == 0 {
        add_stopping_task(thread);
    } else {
        drop(thread);
    }
    // 如果是主线程
    if tid == 0 {
        let pid = process.getpid();
        if pid == IDLE_PID {
            info!("Idle 进程退出，退出码为 {}", exit_code);
            if exit_code != 0 {
                info!("Idle 进程异常退出，正在关闭系统...");
                crate::sbi::panic_shutdown();
            } else {
                info!("Idle 进程正常退出，正在关闭系统...");
                crate::sbi::shutdown();
            }
        }
        remove_from_pid2process(pid);

        let mut process_inner = process.inner_exclusive_access();
        process_inner.is_zombie = true;
        process_inner.exit_code = exit_code;
        {
            let mut initproc_inner = INITPROCESS.inner_exclusive_access();
            for child in process_inner.children.iter() {
                child.inner_exclusive_access().parent = Some(Arc::downgrade(&INITPROCESS));
                initproc_inner.children.push(child.clone());
            }
        }
        let mut recycle_res = Vec::new();
        for thread in process_inner.tasks.iter().filter(|t| t.is_some()) {
            let thread = thread.as_ref().unwrap();
            remove_inactive_task(thread.clone());
            let mut thread_inner = thread.inner_exclusive_access();
            if let Some(res) = thread_inner.res.take() {
                recycle_res.push(res);
            }
        }
        drop(process_inner);
        recycle_res.clear();
        let mut process_inner = process.inner_exclusive_access();
        process_inner.children.clear();
        process_inner.memory_set.recycle_data_pages();
    }
    drop(process);
    let mut _unused = TaskContext::zero_init();
    proc::schedule(&mut _unused as *mut TaskContext);
    panic!("unreachable in exit_current_and_run_next!");
}

// 阻塞当前任务，然后运行下一个任务
pub fn block_current_and_run_next() {
    let task_cx_ptr = mark_current_blocked();
    proc::schedule(task_cx_ptr);
}

pub fn mark_current_blocked() -> *mut TaskContext {
    let thread = proc::current_task();
    let mut thread_inner = thread.inner_exclusive_access();
    let task_cx_ptr = &mut thread_inner.task_cx as *mut TaskContext;
    thread_inner.task_status = TaskStatus::Blocked;
    task_cx_ptr
}

lazy_static::lazy_static! {
    pub static ref INITPROCESS: Arc<ProcessControlBlock> = {
        let inode = open_file("initproc", OpenFlags::RDONLY).unwrap();
        let v = inode.read_all();
        ProcessControlBlock::new(&v)
    };
}

pub fn add_initproc() {
    let _initproc = INITPROCESS.clone();
}

pub fn remove_inactive_task(task: Arc<ThreadControlBlock>) {
    remove_task(task.clone());
    remove_timer(task);
}
