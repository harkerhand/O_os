//! PID
use alloc::{
    sync::{Arc, Weak},
    vec::Vec,
};
use log::{debug, info};

use crate::{
    config::{KERNEL_STACK_SIZE, PAGE_SIZE, TRAMPOLINE, TRAP_CONTEXT_SIZE, USER_STACK_SIZE},
    mem::{KERNEL_SPACE, MapPermission, PhysPageNum, VirtAddr},
    sync::SyncRefCell,
    task::task::ProcessControlBlock,
};

pub const IDLE_PID: usize = 0;

/// 进程 ID
pub struct Pid(pub usize);

pub struct RecycleAllocator {
    current: usize,
    recycle: Vec<usize>,
}

impl RecycleAllocator {
    pub fn new() -> Self {
        Self {
            current: 0,
            recycle: Vec::new(),
        }
    }

    pub fn alloc(&mut self) -> usize {
        if let Some(pid) = self.recycle.pop() {
            pid
        } else {
            let pid = self.current;
            self.current += 1;
            pid
        }
    }

    pub fn dealloc(&mut self, pid: usize) {
        assert!(pid < self.current);
        assert!(!self.recycle.contains(&pid));
        self.recycle.push(pid);
    }
}

lazy_static::lazy_static! {
    static ref PID_ALLOCATOR: SyncRefCell<RecycleAllocator> = unsafe { SyncRefCell::new(RecycleAllocator::new()) };
    static ref KSTACK_ALLOCATOR: SyncRefCell<RecycleAllocator> =
        unsafe { SyncRefCell::new(RecycleAllocator::new()) };
}

pub fn pid_alloc() -> Pid {
    Pid(PID_ALLOCATOR.exclusive_access().alloc())
}

impl Drop for Pid {
    fn drop(&mut self) {
        PID_ALLOCATOR.exclusive_access().dealloc(self.0);
    }
}

/// 获取内核栈的位置，位于 trampoline 的前面，每个线程占用一段连续的内核栈空间
pub fn kernel_stack_position(id: usize) -> (usize, usize) {
    let top = TRAMPOLINE - id * (KERNEL_STACK_SIZE + PAGE_SIZE);
    let bottom = top - KERNEL_STACK_SIZE;
    (bottom, top)
}

/// 获取用户栈的位置
/// 位于 trampoline 的前面
/// trap context 占用一页，用户栈占用 USER_STACK_SIZE
pub fn user_stack_position(tid: usize) -> (usize, usize) {
    let top =
        TRAMPOLINE - TRAP_CONTEXT_SIZE - tid * (USER_STACK_SIZE + TRAP_CONTEXT_SIZE + PAGE_SIZE);
    let bottom = top - USER_STACK_SIZE;
    (bottom, top)
}
/// 获取 trap context 的位置
/// 位于 trampoline 的前面，用户栈的上面，每个线程占用一页
pub fn user_trap_cx_position(tid: usize) -> (usize, usize) {
    let bottom = user_stack_position(tid).1;
    let top = bottom + TRAP_CONTEXT_SIZE;
    (bottom, top)
}

/// 内核栈
pub struct KernelStack(pub usize);

pub fn kstack_alloc() -> KernelStack {
    let kstack_id = KSTACK_ALLOCATOR.exclusive_access().alloc();
    let (kstack_bottom, kstack_top) = kernel_stack_position(kstack_id);
    debug!(
        "分配内核栈 ID = {}, bottom = {:#x}, top = {:#x}",
        kstack_id, kstack_bottom, kstack_top
    );
    KERNEL_SPACE.exclusive_access().insert_framed_area(
        VirtAddr(kstack_bottom),
        VirtAddr(kstack_top),
        MapPermission::R | MapPermission::W,
    );
    KernelStack(kstack_id)
}

impl Drop for KernelStack {
    fn drop(&mut self) {
        let (kernel_stack_bottom, kernel_stack_top) = kernel_stack_position(self.0);
        KERNEL_SPACE
            .exclusive_access()
            .munmap(kernel_stack_bottom, kernel_stack_top);
        KSTACK_ALLOCATOR.exclusive_access().dealloc(self.0);
    }
}

impl KernelStack {
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
        let (_, top) = kernel_stack_position(self.0);
        top
    }
}

pub struct ThreadUserRes {
    pub tid: usize,
    pub process: Weak<ProcessControlBlock>,
}

impl ThreadUserRes {
    pub fn new(process: Arc<ProcessControlBlock>, alloc_user_res: bool) -> Self {
        let tid = process.inner_exclusive_access().alloc_tid();
        let task_user_res = Self {
            tid,
            process: Arc::downgrade(&process),
        };
        if alloc_user_res {
            task_user_res.alloc_user_res();
        }
        task_user_res
    }

    pub fn alloc_user_res(&self) {
        let process = self.process.upgrade().unwrap();
        let mut process_inner = process.inner_exclusive_access();
        // 分配用户栈
        let (user_stack_bottom, user_stack_top) = user_stack_position(self.tid);
        info!(
            "分配用户栈，tid = {}, bottom = {:#x}, top = {:#x}",
            self.tid, user_stack_bottom, user_stack_top
        );
        process_inner.memory_set.insert_framed_area(
            VirtAddr(user_stack_bottom),
            VirtAddr(user_stack_top),
            MapPermission::R | MapPermission::W | MapPermission::U,
        );
        // 分配 trap_cx
        let (trap_cx_bottom, trap_cx_top) = user_trap_cx_position(self.tid);
        info!(
            "分配 trap context，tid = {}, bottom = {:#x}, top = {:#x}",
            self.tid, trap_cx_bottom, trap_cx_top
        );
        process_inner.memory_set.insert_framed_area(
            VirtAddr(trap_cx_bottom),
            VirtAddr(trap_cx_top),
            MapPermission::R | MapPermission::W,
        );
    }

    fn dealloc_user_res(&self) {
        // dealloc tid
        let process = self.process.upgrade().unwrap();
        let mut process_inner = process.inner_exclusive_access();
        // dealloc ustack manually
        let (user_stack_bottom, user_stack_top) = user_stack_position(self.tid);
        process_inner
            .memory_set
            .munmap(user_stack_bottom, user_stack_top);
        // dealloc trap_cx manually
        let (trap_cx_bottom, trap_cx_top) = user_trap_cx_position(self.tid);
        process_inner.memory_set.munmap(trap_cx_bottom, trap_cx_top);
    }

    #[allow(unused)]
    pub fn alloc_tid(&mut self) {
        self.tid = self
            .process
            .upgrade()
            .unwrap()
            .inner_exclusive_access()
            .alloc_tid();
    }

    pub fn dealloc_tid(&self) {
        let process = self.process.upgrade().unwrap();
        let mut process_inner = process.inner_exclusive_access();
        process_inner.dealloc_tid(self.tid);
    }

    pub fn trap_cx_user_va(&self) -> usize {
        user_trap_cx_position(self.tid).0
    }

    pub fn trap_cx_ppn(&self) -> PhysPageNum {
        let process = self.process.upgrade().unwrap();
        let process_inner = process.inner_exclusive_access();
        let trap_cx_bottom_va = VirtAddr(user_trap_cx_position(self.tid).0);
        process_inner
            .memory_set
            .translate(trap_cx_bottom_va.into())
            .unwrap()
            .ppn()
    }

    pub fn ustack_top(&self) -> usize {
        user_stack_position(self.tid).1
    }
}

impl Drop for ThreadUserRes {
    fn drop(&mut self) {
        info!("回收线程资源，tid = {}", self.tid);
        self.dealloc_tid();
        self.dealloc_user_res();
    }
}
