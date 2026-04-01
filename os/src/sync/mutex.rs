use alloc::{collections::vec_deque::VecDeque, sync::Arc};

use crate::{
    sync::SyncRefCell,
    task::{
        ThreadControlBlock, block_current_and_run_next, current_task, suspend_current_and_run_next,
        wakeup_task,
    },
};

pub trait Mutex: Send + Sync {
    fn lock(&self);
    fn unlock(&self);
}

pub struct MutexBlocking {
    inner: SyncRefCell<MutexBlockingInner>,
}

pub struct MutexBlockingInner {
    locked: bool,
    wait_queue: VecDeque<Arc<ThreadControlBlock>>,
}

impl MutexBlocking {
    pub fn new() -> Self {
        Self {
            inner: unsafe {
                SyncRefCell::new(MutexBlockingInner {
                    locked: false,
                    wait_queue: VecDeque::new(),
                })
            },
        }
    }
}

impl Mutex for MutexBlocking {
    fn lock(&self) {
        let mut inner = self.inner.exclusive_access();
        if inner.locked {
            inner.wait_queue.push_back(current_task().unwrap());
            drop(inner);
            block_current_and_run_next();
        } else {
            inner.locked = true;
        }
    }

    fn unlock(&self) {
        let mut inner = self.inner.exclusive_access();
        if let Some(task) = inner.wait_queue.pop_front() {
            wakeup_task(task);
        } else {
            inner.locked = false;
        }
    }
}

pub struct MutexSpin {
    locked: SyncRefCell<bool>,
}

impl MutexSpin {
    pub fn new() -> Self {
        Self {
            locked: unsafe { SyncRefCell::new(false) },
        }
    }
}

impl Mutex for MutexSpin {
    fn lock(&self) {
        loop {
            let mut locked = self.locked.exclusive_access();
            if !*locked {
                *locked = true;
                break;
            }
            drop(locked);
            suspend_current_and_run_next();
        }
    }

    fn unlock(&self) {
        let mut locked = self.locked.exclusive_access();
        *locked = false;
    }
}
