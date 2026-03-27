//! 进程管理器

use alloc::{collections::vec_deque::VecDeque, sync::Arc};

use crate::{sync::SyncRefCell, task::task::ProcessControlBlock};

pub struct TaskManager {
    ready_queue: VecDeque<Arc<ProcessControlBlock>>,
}

impl TaskManager {
    pub fn new() -> Self {
        Self {
            ready_queue: VecDeque::new(),
        }
    }
    pub fn add(&mut self, pcb: Arc<ProcessControlBlock>) {
        self.ready_queue.push_back(pcb);
    }
    pub fn fetch(&mut self) -> Option<Arc<ProcessControlBlock>> {
        self.ready_queue.pop_front()
    }
}

lazy_static::lazy_static! {
    pub static ref TASK_MANAGER: SyncRefCell<TaskManager> = unsafe { SyncRefCell::new(TaskManager::new()) };
}

pub fn add_task(pcb: Arc<ProcessControlBlock>) {
    TASK_MANAGER.exclusive_access().add(pcb);
}

pub fn fetch_task() -> Option<Arc<ProcessControlBlock>> {
    TASK_MANAGER.exclusive_access().fetch()
}
