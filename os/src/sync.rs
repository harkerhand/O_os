//! 为 RefCell 提供一个不安全的版本，用户需要保证在单核处理器上使用它。

use core::cell::{RefCell, RefMut};
pub struct SyncRefCell<T> {
    /// inner data
    inner: RefCell<T>,
}

unsafe impl<T> Sync for SyncRefCell<T> {}

impl<T> SyncRefCell<T> {
    pub unsafe fn new(value: T) -> Self {
        Self {
            inner: RefCell::new(value),
        }
    }
    pub fn exclusive_access(&self) -> RefMut<'_, T> {
        self.inner.borrow_mut()
    }
}
