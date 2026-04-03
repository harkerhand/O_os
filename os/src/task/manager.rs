//! 进程管理器

use alloc::{
    collections::{btree_map::BTreeMap, vec_deque::VecDeque},
    sync::Arc,
    vec::Vec,
};

use crate::{
    sync::SyncRefCell,
    task::task::{ProcessControlBlock, TaskStatus, ThreadControlBlock},
};

pub struct TaskManager {
    ready_queue: VecDeque<Arc<ThreadControlBlock>>,
    stopped_tasks: Vec<Arc<ThreadControlBlock>>,
}

impl TaskManager {
    pub fn new() -> Self {
        Self {
            ready_queue: VecDeque::new(),
            stopped_tasks: Vec::new(),
        }
    }
    pub fn add(&mut self, pcb: Arc<ThreadControlBlock>) {
        self.ready_queue.push_back(pcb);
    }
    pub fn fetch(&mut self) -> Option<Arc<ThreadControlBlock>> {
        // Exited tasks must stay alive until we are back on the idle context.
        // Once the scheduler loop runs again, dropping them is safe.
        self.stopped_tasks.clear();
        self.ready_queue.pop_front()
    }
    pub fn add_stop(&mut self, task: Arc<ThreadControlBlock>) {
        self.stopped_tasks.push(task);
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
    if inner.task_status != TaskStatus::Blocked {
        return;
    }
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

pub fn pid2process(pid: usize) -> Option<Arc<ProcessControlBlock>> {
    PID2PCB.exclusive_access().get(&pid).cloned()
}

pub fn insert_into_pid2process(pid: usize, process: Arc<ProcessControlBlock>) {
    PID2PCB.exclusive_access().insert(pid, process);
}

pub fn remove_from_pid2process(pid: usize) {
    if PID2PCB.exclusive_access().remove(&pid).is_none() {
        log::warn!("移除进程映射失败: 找不到 PID {}", pid);
    }
}
