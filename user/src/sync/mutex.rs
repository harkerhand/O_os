use core::{
    cell::UnsafeCell,
    ops::{Deref, DerefMut},
};

use crate::{mutex_blocking_create, mutex_lock, mutex_unlock};

#[derive(Debug)]
pub struct Mutex<T> {
    lock_id: usize,
    data: UnsafeCell<T>,
}

pub struct MutexGuard<'a, T> {
    mutex: &'a Mutex<T>,
}

impl<T> Mutex<T> {
    pub fn new(user_data: T) -> Self {
        let lock_id = mutex_blocking_create() as usize;
        Self {
            lock_id,
            data: UnsafeCell::new(user_data),
        }
    }

    pub fn lock(&self) -> MutexGuard<'_, T> {
        mutex_lock(self.lock_id);
        MutexGuard { mutex: self }
    }
}

impl<'a, T> Deref for MutexGuard<'a, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.mutex.data.get() }
    }
}

impl<'a, T> DerefMut for MutexGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.mutex.data.get() }
    }
}

impl<'a, T> Drop for MutexGuard<'a, T> {
    fn drop(&mut self) {
        mutex_unlock(self.mutex.lock_id);
    }
}

unsafe impl<T> Sync for Mutex<T> {}
