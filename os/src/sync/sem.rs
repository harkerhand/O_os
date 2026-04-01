use alloc::{collections::vec_deque::VecDeque, sync::Arc};

use crate::{
    sync::SyncRefCell,
    task::{ThreadControlBlock, wakeup_task},
};

pub struct Semaphore {
    pub inner: SyncRefCell<SemaphoreInner>,
}

pub struct SemaphoreInner {
    count: isize,
    wait_queue: VecDeque<Arc<ThreadControlBlock>>,
}

impl Semaphore {
    pub fn new(count: isize) -> Self {
        Self {
            inner: unsafe {
                SyncRefCell::new(SemaphoreInner {
                    count,
                    wait_queue: VecDeque::new(),
                })
            },
        }
    }

    pub fn up(&self) {
        let mut inner = self.inner.exclusive_access();
        inner.count += 1;
        if inner.count <= 0
            && let Some(task) = inner.wait_queue.pop_front()
        {
            wakeup_task(task);
        }
    }

    pub fn down(&self) {
        let mut inner = self.inner.exclusive_access();
        inner.count -= 1;
        if inner.count < 0 {
            inner
                .wait_queue
                .push_back(crate::task::current_task().unwrap());
            drop(inner);
            crate::task::block_current_and_run_next();
        }
    }
}
