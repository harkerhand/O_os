//! 任务管理模块，负责管理和调度用户态的任务

mod context;
mod switch;

mod pid;
#[allow(clippy::module_inception)]
mod task;

use crate::fs::inode::{OpenFlags, open_file};
use crate::task::manager::{add_stopping_task, remove_from_pid2process};
use crate::task::pid::IDLE_PID;
use crate::task::proc::{schedule, take_current_task};
use crate::task::task::ProcessControlBlock;
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
    let task = take_current_task().unwrap();

    let mut task_inner = task.inner_exclusive_access();
    let task_cx_ptr = &mut task_inner.task_cx as *mut TaskContext;
    task_inner.task_status = TaskStatus::Ready;
    drop(task_inner);

    add_task(task);
    schedule(task_cx_ptr);
}

/// 退出当前任务，然后运行下一个任务
pub fn exit_current_and_run_next(exit_code: i32) {
    let thread = take_current_task().unwrap();
    let mut thread_inner = thread.inner_exclusive_access();
    let process = thread.process.upgrade().unwrap();
    let tid = thread_inner.res.as_ref().unwrap().tid;
    thread_inner.exit_code = Some(exit_code);
    thread_inner.res = None;
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
    schedule(&mut _unused as *mut TaskContext);
    panic!("unreachable in exit_current_and_run_next!");
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
