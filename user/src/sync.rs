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
        // 自旋：尝试将 false 改为 true
        // Acquire 内存顺序确保锁之后的内存操作不会重排到锁之前
        let mut retry_count = 100;
        while self
            .locked
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            retry_count -= 1;
            if retry_count == 0 {
                sys_yield(); // 让出 CPU，避免忙等待过热
                retry_count = 100;
            } else {
                core::hint::spin_loop(); // 提示 CPU 这是一个自旋等待
            }
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
