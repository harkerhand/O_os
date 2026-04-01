use core::cell::UnsafeCell;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::{AtomicBool, Ordering};

use crate::syscall::sys_yield;

pub struct SpinLock<T> {
    locked: AtomicBool,
    data: UnsafeCell<T>,
}

unsafe impl<T> Sync for SpinLock<T> {}

pub struct SpinLockGuard<'a, T> {
    spin_lock: &'a SpinLock<T>,
}

impl<T> SpinLock<T> {
    pub const fn new(user_data: T) -> Self {
        Self {
            locked: AtomicBool::new(false),
            data: UnsafeCell::new(user_data),
        }
    }

    pub fn lock(&self) -> SpinLockGuard<'_, T> {
        while self.locked.swap(true, Ordering::Acquire) {
            // 如果已经被锁定，主动让出 CPU，避免忙等待
            sys_yield();
        }
        SpinLockGuard { spin_lock: self }
    }

    // 释放锁由 Guard 的 Drop 实现
    fn unlock(&self) {
        // Release 内存顺序确保锁之前的内存操作已经完成
        self.locked.store(false, Ordering::Release);
    }
}

impl<'a, T> Deref for SpinLockGuard<'a, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.spin_lock.data.get() }
    }
}

impl<'a, T> DerefMut for SpinLockGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.spin_lock.data.get() }
    }
}

impl<'a, T> Drop for SpinLockGuard<'a, T> {
    fn drop(&mut self) {
        self.spin_lock.unlock();
    }
}
