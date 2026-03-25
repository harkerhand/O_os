//! 任务管理模块，负责管理和调度用户态的任务

mod context;
mod switch;

#[allow(clippy::module_inception)]
mod task;

use crate::config::MAX_APP_NUM;
use crate::loader::{get_num_app, init_app_cx};
use crate::sync::SyncRefCell;
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
    tasks: [TaskControlBlock; MAX_APP_NUM],
    /// 当前正在运行的任务 id
    current_task: usize,
}

lazy_static! {
    pub static ref TASK_MANAGER: TaskManager = {
        let num_app = get_num_app();
        let mut tasks = [TaskControlBlock {
            task_cx: TaskContext::zero_init(),
            task_status: TaskStatus::UnInit,
        }; MAX_APP_NUM];
        // 初始化任务列表
        for (i, task) in tasks.iter_mut().enumerate() {
            task.task_cx = TaskContext::goto_restore(init_app_cx(i));
            task.task_status = TaskStatus::Ready;
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
        trace!("[kernel] 运行第一个任务");
        let mut inner = self.inner.exclusive_access();
        let task0 = &mut inner.tasks[0];
        task0.task_status = TaskStatus::Running;
        let next_task_cx_ptr = &task0.task_cx as *const TaskContext;
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
        trace!("[kernel] 挂起当前任务: {}", current);
        inner.tasks[current].task_status = TaskStatus::Ready;
    }

    /// 将当前 `Running` 的任务状态改为 `Exited`
    fn mark_current_exited(&self) {
        let mut inner = self.inner.exclusive_access();
        let current = inner.current_task;
        trace!("[kernel] 标记当前任务为已退出: {}", current);
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

    /// 切换到下一个任务，如果没有 `Ready` 的任务了，就关机
    fn run_next_task(&self) {
        if let Some(next) = self.find_next_task() {
            let mut inner = self.inner.exclusive_access();
            let current = inner.current_task;
            trace!("[kernel] 切换任务: {} -> {}", current, next);
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
            info!("[kernel] 所有应用已完成，准备关机");
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
