//! 任务管理模块，负责管理和调度用户态的任务

mod context;
mod switch;

#[allow(clippy::module_inception)]
mod task;

use crate::loader::get_app_data;
use crate::loader::get_num_app;
use crate::sync::SyncRefCell;
use crate::trap::TrapContext;
use alloc::vec::Vec;
use lazy_static::*;
use log::{info, trace};
use switch::__switch;
use task::{TaskControlBlock, TaskStatus};

pub use context::TaskContext;

/// 任务管理器
pub struct TaskManager {
    num_app: usize,
    inner: SyncRefCell<TaskManagerInner>,
}

/// 任务管理器内部数据结构
pub struct TaskManagerInner {
    /// 任务列表
    tasks: Vec<TaskControlBlock>,
    /// 当前正在运行的任务 id
    current_task: usize,
}

lazy_static! {
    pub static ref TASK_MANAGER: TaskManager = {
        info!("初始化任务管理器");
        let num_app = get_num_app();
        info!("应用数量 = {}", num_app);
        let mut tasks: Vec<TaskControlBlock> = Vec::new();
        for i in 0..num_app {
            tasks.push(TaskControlBlock::new(get_app_data(i), i));
        }
        TaskManager {
            num_app,
            inner: unsafe {
                SyncRefCell::new(TaskManagerInner {
                    tasks,
                    current_task: 0,
                })
            },
        }
    };
}

impl TaskManager {
    /// 运行第一个任务
    fn run_first_task(&self) -> ! {
        trace!("运行第一个任务");
        let mut inner = self.inner.exclusive_access();
        let next_task = &mut inner.tasks[0];
        next_task.task_status = TaskStatus::Running;
        let next_task_cx_ptr = &next_task.task_cx as *const TaskContext;
        drop(inner);
        let mut _unused = TaskContext::zero_init();
        unsafe {
            __switch(&mut _unused as *mut TaskContext, next_task_cx_ptr);
        }
        panic!("unreachable in run_first_task!");
    }

    /// 将当前 `Running` 的任务状态改为 `Ready`
    fn mark_current_suspended(&self) {
        let mut inner = self.inner.exclusive_access();
        let current = inner.current_task;
        trace!("挂起当前任务: {}", current);
        inner.tasks[current].task_status = TaskStatus::Ready;
    }

    /// 将当前 `Running` 的任务状态改为 `Exited`
    fn mark_current_exited(&self) {
        let mut inner = self.inner.exclusive_access();
        let current = inner.current_task;
        trace!("标记当前任务为已退出: {}", current);
        inner.tasks[current].task_status = TaskStatus::Exited;
    }

    /// 寻找下一个 `Ready` 的任务，如果没有了就返回 `None`
    fn find_next_task(&self) -> Option<usize> {
        let inner = self.inner.exclusive_access();
        let current = inner.current_task;
        (current + 1..current + self.num_app + 1)
            .map(|id| id % self.num_app)
            .find(|id| inner.tasks[*id].task_status == TaskStatus::Ready)
    }

    /// 获取当前 `Running` 的任务的用户空间 token
    fn get_current_token(&self) -> usize {
        let inner = self.inner.exclusive_access();
        inner.tasks[inner.current_task].get_user_token()
    }

    /// 获取当前 `Running` 的任务的 trap context
    fn get_current_trap_cx(&self) -> &'static mut TrapContext {
        let inner = self.inner.exclusive_access();
        inner.tasks[inner.current_task].get_trap_cx()
    }

    /// 改变当前 `Running` 的任务的程序 break 的位置，返回新的程序 break，如果失败了就返回 `None`
    pub fn change_current_program_brk(&self, size: i32) -> Option<usize> {
        let mut inner = self.inner.exclusive_access();
        let cur = inner.current_task;
        inner.tasks[cur].change_program_brk(size)
    }

    pub fn mmap_current(&self, start: usize, end: usize, prot: usize) -> isize {
        let mut inner = self.inner.exclusive_access();
        let cur = inner.current_task;
        inner.tasks[cur].mmap(start, end, prot)
    }

    pub fn munmap_current(&self, start: usize, end: usize) -> isize {
        let mut inner = self.inner.exclusive_access();
        let cur = inner.current_task;
        inner.tasks[cur].munmap(start, end)
    }

    /// 切换到下一个任务，如果没有 `Ready` 的任务了，就关机
    fn run_next_task(&self) {
        if let Some(next) = self.find_next_task() {
            let mut inner = self.inner.exclusive_access();
            let current = inner.current_task;
            trace!("切换任务: {} -> {}", current, next);
            inner.tasks[next].task_status = TaskStatus::Running;
            inner.current_task = next;
            let current_task_cx_ptr = &mut inner.tasks[current].task_cx as *mut TaskContext;
            let next_task_cx_ptr = &inner.tasks[next].task_cx as *const TaskContext;
            drop(inner);

            unsafe {
                __switch(current_task_cx_ptr, next_task_cx_ptr);
            }
            // 返回用户态
        } else {
            info!("所有应用已完成，准备关机");
            crate::sbi::shutdown();
        }
    }
}

/// 运行第一个任务
pub fn run_first_task() {
    TASK_MANAGER.run_first_task();
}

/// 运行下一个任务
fn run_next_task() {
    TASK_MANAGER.run_next_task();
}

/// 挂起当前任务
fn mark_current_suspended() {
    TASK_MANAGER.mark_current_suspended();
}

/// 标记当前任务为已退出
fn mark_current_exited() {
    TASK_MANAGER.mark_current_exited();
}

/// 挂起当前任务，然后运行下一个任务
pub fn suspend_current_and_run_next() {
    mark_current_suspended();
    run_next_task();
}

/// 退出当前任务，然后运行下一个任务
pub fn exit_current_and_run_next() {
    mark_current_exited();
    run_next_task();
}

/// Get the current 'Running' task's token.
pub fn current_user_token() -> usize {
    TASK_MANAGER.get_current_token()
}

/// Get the current 'Running' task's trap contexts.
pub fn current_trap_cx() -> &'static mut TrapContext {
    TASK_MANAGER.get_current_trap_cx()
}

/// Change the current 'Running' task's program break
pub fn change_program_brk(size: i32) -> Option<usize> {
    TASK_MANAGER.change_current_program_brk(size)
}

pub fn mmap_current(start: usize, end: usize, prot: usize) -> isize {
    TASK_MANAGER.mmap_current(start, end, prot)
}

pub fn munmap_current(start: usize, end: usize) -> isize {
    TASK_MANAGER.munmap_current(start, end)
}
