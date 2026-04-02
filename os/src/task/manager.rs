//! 进程管理器

use alloc::{
    collections::{btree_map::BTreeMap, vec_deque::VecDeque},
    sync::Arc,
};

use crate::{
    sync::SyncRefCell,
    task::task::{ProcessControlBlock, TaskStatus, ThreadControlBlock},
};

pub struct TaskManager {
    ready_queue: VecDeque<Arc<ThreadControlBlock>>,
    stop_task: Option<Arc<ThreadControlBlock>>,
}

impl TaskManager {
    pub fn new() -> Self {
        Self {
            ready_queue: VecDeque::new(),
            stop_task: None,
        }
    }
    pub fn add(&mut self, pcb: Arc<ThreadControlBlock>) {
        self.ready_queue.push_back(pcb);
    }
    pub fn fetch(&mut self) -> Option<Arc<ThreadControlBlock>> {
        self.ready_queue.pop_front()
    }
    pub fn add_stop(&mut self, task: Arc<ThreadControlBlock>) {
        self.stop_task = Some(task);
    }
    pub fn remove(&mut self, task: Arc<ThreadControlBlock>) {
        if let Some(id) = self.ready_queue.iter().position(|t| Arc::ptr_eq(t, &task)) {
            self.ready_queue.remove(id);
        }
    }
}

lazy_static::lazy_static! {
    pub static ref TASK_MANAGER: SyncRefCell<TaskManager> = unsafe { SyncRefCell::new(TaskManager::new()) };
    pub static ref PID2PCB: SyncRefCell<BTreeMap<usize, Arc<ProcessControlBlock>>> = unsafe { SyncRefCell::new(BTreeMap::new()) };
}

pub fn add_task(pcb: Arc<ThreadControlBlock>) {
    TASK_MANAGER.exclusive_access().add(pcb);
}

pub fn fetch_task() -> Option<Arc<ThreadControlBlock>> {
    TASK_MANAGER.exclusive_access().fetch()
}

pub fn wakeup_task(task: Arc<ThreadControlBlock>) {
    let mut inner = task.inner_exclusive_access();
    inner.task_status = TaskStatus::Ready;
    drop(inner);
    add_task(task);
}

pub fn remove_task(task: Arc<ThreadControlBlock>) {
    TASK_MANAGER.exclusive_access().remove(task);
}

pub fn add_stopping_task(task: Arc<ThreadControlBlock>) {
    TASK_MANAGER.exclusive_access().add_stop(task);
}

pub fn insert_into_pid2process(pid: usize, process: Arc<ProcessControlBlock>) {
    PID2PCB.exclusive_access().insert(pid, process);
}

pub fn remove_from_pid2process(pid: usize) {
    PID2PCB
        .exclusive_access()
        .remove(&pid)
        .unwrap_or_else(|| panic!("找不到 PID 为 {} 的进程", pid));
}
