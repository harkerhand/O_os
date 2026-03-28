//! 任务管理模块，负责管理和调度用户态的任务

mod context;
mod switch;

mod pid;
#[allow(clippy::module_inception)]
mod task;

use crate::fs::inode::{OpenFlags, open_file};
use crate::task::proc::{schedule, take_current_task};
use crate::task::task::ProcessControlBlock;
use alloc::sync::Arc;
use task::TaskStatus;
mod manager;
mod proc;

pub use context::TaskContext;
pub use manager::add_task;
pub use proc::{
    change_program_brk, current_task, current_trap_cx, current_user_token, mmap_current,
    munmap_current, run,
};

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
    let process = take_current_task().unwrap();
    let mut current_tcb = process.inner_exclusive_access();
    current_tcb.task_status = TaskStatus::Zombie;
    current_tcb.exit_code = exit_code;
    {
        let mut initproc_inner = INITPROCESS.inner_exclusive_access();
        for child in current_tcb.children.iter() {
            child.inner_exclusive_access().parent = Some(Arc::downgrade(&INITPROCESS));
            initproc_inner.children.push(child.clone());
        }
    }
    current_tcb.children.clear();
    current_tcb.memory_set.recycle_data_pages();
    drop(current_tcb);
    drop(process);
    let mut _unused = TaskContext::zero_init();
    schedule(&mut _unused as *mut TaskContext);
    panic!("unreachable in exit_current_and_run_next!");
}

lazy_static::lazy_static! {
    pub static ref INITPROCESS: Arc<ProcessControlBlock> = Arc::new({
        let inode = open_file("initproc", OpenFlags::RDONLY).unwrap();
        let v = inode.read_all();
        ProcessControlBlock::new(&v)
    });
}

pub fn add_initproc() {
    manager::add_task(INITPROCESS.clone());
}
