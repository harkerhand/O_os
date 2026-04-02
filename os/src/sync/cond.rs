use alloc::{collections::vec_deque::VecDeque, sync::Arc};

use crate::{
    sync::{Mutex, SyncRefCell},
    task::{ThreadControlBlock, current_task, mark_current_blocked, schedule, wakeup_task},
};

pub struct Condvar {
    pub inner: SyncRefCell<CondvarInner>,
}

pub struct CondvarInner {
    pub wait_queue: VecDeque<Arc<ThreadControlBlock>>,
}

impl Condvar {
    pub fn new() -> Self {
        Self {
            inner: unsafe {
                SyncRefCell::new(CondvarInner {
                    wait_queue: VecDeque::new(),
                })
            },
        }
    }

    pub fn signal(&self) {
        let mut inner = self.inner.exclusive_access();
        if let Some(thread) = inner.wait_queue.pop_front() {
            wakeup_task(thread);
        }
    }

    pub fn wait(&self, mutex: Arc<dyn Mutex>) {
        let current_thread = current_task();
        let task_cx_ptr = mark_current_blocked();
        let mut inner = self.inner.exclusive_access();
        inner.wait_queue.push_back(current_thread.clone());
        mutex.unlock();
        drop(inner);
        drop(current_thread);
        schedule(task_cx_ptr);
        mutex.lock();
    }
}
