//! 进程

use alloc::sync::Arc;

use crate::{
    error::KernelResult,
    sync::SyncRefCell,
    task::{
        TaskContext, add_initproc,
        manager::fetch_task,
        switch::__switch,
        task::{ProcessControlBlock, TaskStatus},
    },
    trap::TrapContext,
};

pub struct ProcessorManager {
    current: Option<Arc<ProcessControlBlock>>,
    idle_task_cx: TaskContext,
}

lazy_static::lazy_static! {
    pub static ref PROCESSOR: SyncRefCell<ProcessorManager> = unsafe { SyncRefCell::new(ProcessorManager::new()) };
}

impl ProcessorManager {
    pub fn take_current(&mut self) -> Option<Arc<ProcessControlBlock>> {
        self.current.take()
    }
    pub fn current(&self) -> Option<Arc<ProcessControlBlock>> {
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

pub fn take_current_task() -> Option<Arc<ProcessControlBlock>> {
    PROCESSOR.exclusive_access().take_current()
}

pub fn current_task() -> Option<Arc<ProcessControlBlock>> {
    PROCESSOR.exclusive_access().current()
}

pub fn current_user_token() -> usize {
    let task = current_task().unwrap();
    task.inner_exclusive_access().get_user_token()
}

pub fn current_trap_cx() -> &'static mut TrapContext {
    current_task()
        .unwrap()
        .inner_exclusive_access()
        .get_trap_cx()
}

pub fn change_program_brk(size: i32) -> KernelResult<usize> {
    current_task()
        .unwrap()
        .inner_exclusive_access()
        .change_program_brk(size)
}

pub fn mmap_current(start: usize, end: usize, prot: usize) -> isize {
    current_task()
        .unwrap()
        .inner_exclusive_access()
        .mmap(start, end, prot)
}

pub fn munmap_current(start: usize, end: usize) -> isize {
    current_task()
        .unwrap()
        .inner_exclusive_access()
        .munmap(start, end)
}

pub fn run() {
    add_initproc();
    loop {
        let mut processer_manager = PROCESSOR.exclusive_access();
        if let Some(task) = fetch_task() {
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
