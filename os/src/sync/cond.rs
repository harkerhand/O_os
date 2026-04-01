use alloc::{collections::vec_deque::VecDeque, sync::Arc};

use crate::{
    sync::{Mutex, SyncRefCell},
    task::{ThreadControlBlock, block_current_and_run_next, current_task, wakeup_task},
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
        mutex.unlock();
        let current_thread = current_task().unwrap();
        {
            let mut inner = self.inner.exclusive_access();
            inner.wait_queue.push_back(current_thread.clone());
        }
        drop(current_thread);
        block_current_and_run_next();
        mutex.lock();
    }
}
