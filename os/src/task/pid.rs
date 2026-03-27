//! PID
use alloc::vec::Vec;

use crate::{
    config::{KERNEL_STACK_SIZE, PAGE_SIZE, TRAMPOLINE},
    mem::{KERNEL_SPACE, MapPermission, VirtAddr},
    sync::SyncRefCell,
};

/// 进程 ID
pub struct Pid(pub usize);

struct PidAllocator {
    current: usize,
    recycle: Vec<usize>,
}

impl PidAllocator {
    fn new() -> Self {
        Self {
            current: 0,
            recycle: Vec::new(),
        }
    }

    fn alloc(&mut self) -> Pid {
        if let Some(pid) = self.recycle.pop() {
            Pid(pid)
        } else {
            let pid = self.current;
            self.current += 1;
            Pid(pid)
        }
    }

    fn dealloc(&mut self, pid: usize) {
        assert!(pid < self.current);
        assert!(!self.recycle.contains(&pid));
        self.recycle.push(pid);
    }
}

lazy_static::lazy_static! {
    static ref PID_ALLOCATOR: SyncRefCell<PidAllocator> = unsafe { SyncRefCell::new(PidAllocator::new()) };
}

pub fn pid_alloc() -> Pid {
    PID_ALLOCATOR.exclusive_access().alloc()
}

impl Drop for Pid {
    fn drop(&mut self) {
        PID_ALLOCATOR.exclusive_access().dealloc(self.0);
    }
}

/// 获取内核栈的位置，位于 trampoline 的前面，每个应用程序占用一段连续的内核栈空间
pub fn kernel_stack_position(app_id: usize) -> (usize, usize) {
    let top = TRAMPOLINE - app_id * (KERNEL_STACK_SIZE + PAGE_SIZE);
    let bottom = top - KERNEL_STACK_SIZE;
    (bottom, top)
}

/// 内核栈
pub struct KernelStack {
    pid: usize,
}

impl KernelStack {
    pub fn new(pid: &Pid) -> Self {
        let pid = pid.0;
        let (bottom, top) = kernel_stack_position(pid);
        KERNEL_SPACE.exclusive_access().insert_framed_area(
            VirtAddr(bottom),
            VirtAddr(top),
            MapPermission::R | MapPermission::W,
        );
        Self { pid }
    }

    #[allow(unused)]
    pub fn push_on_top<T>(&self, value: T) -> *mut T
    where
        T: Sized,
    {
        let top = self.get_top();
        let ptr = (top - core::mem::size_of::<T>()) as *mut T;
        unsafe {
            *ptr = value;
        }
        ptr
    }

    pub fn get_top(&self) -> usize {
        let (_, top) = kernel_stack_position(self.pid);
        top
    }
}

impl Drop for KernelStack {
    fn drop(&mut self) {
        let (bottom, top) = kernel_stack_position(self.pid);
        assert_eq!(KERNEL_SPACE.exclusive_access().munmap(bottom, top), 0);
    }
}
