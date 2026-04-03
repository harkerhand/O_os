//! 进程

use alloc::sync::Arc;
use log::info;

use crate::{
    error::KernelResult,
    sync::SyncRefCell,
    task::{
        TaskContext, add_initproc,
        manager::fetch_task,
        switch::__switch,
        task::{ProcessControlBlock, TaskStatus, ThreadControlBlock},
    },
    trap::TrapContext,
};

pub struct ProcessorManager {
    current: Option<Arc<ThreadControlBlock>>,
    idle_task_cx: TaskContext,
}

lazy_static::lazy_static! {
    pub static ref PROCESSOR: SyncRefCell<ProcessorManager> = unsafe { SyncRefCell::new(ProcessorManager::new()) };
}

impl ProcessorManager {
    pub fn take_current(&mut self) -> Option<Arc<ThreadControlBlock>> {
        self.current.take()
    }
    pub fn current(&self) -> Option<Arc<ThreadControlBlock>> {
        self.current.as_ref().cloned()
    }

    pub fn new() -> Self {
        Self {
            current: None,
            idle_task_cx: TaskContext::zero_init(),
        }
    }
    fn get_idle_task_cx(&mut self) -> *mut TaskContext {
        &mut self.idle_task_cx as *mut TaskContext
    }
}

pub fn take_current_task() -> Arc<ThreadControlBlock> {
    PROCESSOR
        .exclusive_access()
        .take_current()
        .expect("no current task to take")
}

pub fn current_task() -> Arc<ThreadControlBlock> {
    try_current_task().expect("no current task")
}

pub fn try_current_task() -> Option<Arc<ThreadControlBlock>> {
    PROCESSOR.exclusive_access().current()
}

pub fn current_process() -> Arc<ProcessControlBlock> {
    try_current_process().expect("no current process")
}

pub fn try_current_process() -> Option<Arc<ProcessControlBlock>> {
    try_current_task().and_then(|task| task.process.upgrade())
}

pub fn current_user_token() -> usize {
    current_process().inner_exclusive_access().get_user_token()
}

pub fn current_trap_cx() -> &'static mut TrapContext {
    try_current_task()
        .expect("no current task for trap context")
        .inner_exclusive_access()
        .get_trap_cx()
}

pub fn current_trap_cx_user_va() -> usize {
    try_current_task()
        .expect("no current task for trap context user va")
        .inner_exclusive_access()
        .res
        .as_ref()
        .expect("current task has no user resources")
        .trap_cx_user_va()
}

pub fn change_program_brk(size: i32) -> KernelResult<usize> {
    current_process()
        .inner_exclusive_access()
        .change_program_brk(size)
}

pub fn mmap_current(start: usize, end: usize, prot: usize) -> isize {
    match current_process()
        .inner_exclusive_access()
        .mmap(start, end, prot)
    {
        Ok(()) => 0,
        Err(_) => -1,
    }
}

pub fn munmap_current(start: usize, end: usize) -> isize {
    match current_process()
        .inner_exclusive_access()
        .munmap(start, end)
    {
        Ok(()) => 0,
        Err(_) => -1,
    }
}

pub fn run() {
    add_initproc();
    info!("开始调度");
    loop {
        if let Some(task) = fetch_task() {
            let mut processer_manager = PROCESSOR.exclusive_access();
            let idle_task_cx_ptr = processer_manager.get_idle_task_cx();
            let mut task_inner = task.inner_exclusive_access();
            let next_task_cx_ptr = &task_inner.task_cx as *const TaskContext;
            task_inner.task_status = TaskStatus::Running;
            drop(task_inner);
            processer_manager.current = Some(task);
            drop(processer_manager);
            unsafe {
                __switch(idle_task_cx_ptr, next_task_cx_ptr);
            }
        }
    }
}

pub fn schedule(switched_task_cx_ptr: *mut TaskContext) {
    let mut processor_manager = PROCESSOR.exclusive_access();
    let idle_task_cx_ptr = processor_manager.get_idle_task_cx();
    drop(processor_manager);
    unsafe {
        __switch(switched_task_cx_ptr, idle_task_cx_ptr);
    }
}
