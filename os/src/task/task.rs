//! 任务控制块

use core::cell::RefMut;

use alloc::{
    string::String,
    sync::{Arc, Weak},
    vec,
    vec::Vec,
};
use log::debug;

use crate::{
    error::{KernelError, KernelResult},
    fs::File,
    mem::{KERNEL_SPACE, MemorySet, PhysPageNum, VirtAddr, translated_refmut},
    sync::{Condvar, Mutex, Semaphore, SyncRefCell},
    task::{
        SignalFlags, TaskContext, add_task,
        manager::insert_into_pid2process,
        pid::{
            self, KernelStack, Pid, RecycleAllocator, ThreadUserRes, kstack_alloc, pid_alloc,
            user_stack_position,
        },
    },
    trap::{TrapContext, trap_handler},
};

/// 进程控制块
pub struct ProcessControlBlock {
    pub pid: Pid,
    inner: SyncRefCell<ProcessControlBlockInner>,
}

impl ProcessControlBlock {
    pub fn inner_exclusive_access(&self) -> RefMut<'_, ProcessControlBlockInner> {
        self.inner.exclusive_access()
    }
    pub fn new(elf_data: &[u8]) -> Arc<Self> {
        let (memory_set, heap_bottom, entry_point) = MemorySet::from_elf(elf_data);
        let pid = pid_alloc();
        let process = Arc::new(Self {
            pid,
            inner: unsafe {
                SyncRefCell::new(ProcessControlBlockInner {
                    is_zombie: false,
                    memory_set,
                    parent: None,
                    children: Vec::new(),
                    exit_code: 0,
                    fd_table: vec![
                        Some(Arc::new(crate::fs::stdio::Stdin)),
                        Some(Arc::new(crate::fs::stdio::Stdout)),
                        Some(Arc::new(crate::fs::stdio::Stderr)),
                    ],
                    tasks: Vec::new(),
                    task_res_allocator: RecycleAllocator::new(),
                    sem_list: Vec::new(),
                    mutex_list: Vec::new(),
                    cond_list: Vec::new(),
                    signals: SignalFlags::empty(),
                    heap_bottom,
                    program_brk: heap_bottom,
                    cwd: String::from("/"),
                })
            },
        });
        debug!(
            "创建进程 PID = {}, entry_point = {:#x}",
            process.pid.0, entry_point
        );
        let thread = Arc::new(ThreadControlBlock::new(Arc::clone(&process), true));
        let thread_inner = thread.inner_exclusive_access();
        let trap_cx = thread_inner.get_trap_cx();
        let tid = thread_inner.res.as_ref().unwrap().tid;
        let user_stack_top = user_stack_position(tid).1;
        let kernel_stack_top = thread.kernel_stack.get_top();
        debug!(
            "创建线程 TID = {}, kernel_stack_top = {:#x}",
            thread_inner.trap_cx_ppn.0, kernel_stack_top
        );
        drop(thread_inner);
        *trap_cx = TrapContext::app_init_context(
            entry_point,
            user_stack_top,
            KERNEL_SPACE.exclusive_access().token(),
            kernel_stack_top,
            trap_handler as *const () as usize,
        );
        let mut process_inner = process.inner_exclusive_access();
        process_inner.tasks.push(Some(thread.clone()));
        drop(process_inner);
        insert_into_pid2process(process.getpid(), process.clone());
        add_task(thread);
        process
    }

    pub fn exec(self: &Arc<Self>, elf_data: &[u8], args: Vec<String>) {
        assert_eq!(self.inner.exclusive_access().thread_count(), 1);
        let (memory_set, heap_bottom, entry_point) = MemorySet::from_elf(elf_data);
        let new_token = memory_set.token();
        let mut self_inner = self.inner_exclusive_access();
        self_inner.memory_set = memory_set;
        self_inner.heap_bottom = heap_bottom;
        self_inner.program_brk = heap_bottom;
        drop(self_inner);
        let thread = self.inner_exclusive_access().get_task(0);
        let mut thread_inner = thread.inner_exclusive_access();
        thread_inner.res.as_mut().unwrap().alloc_user_res();
        thread_inner.trap_cx_ppn = thread_inner.res.as_ref().unwrap().trap_cx_ppn();
        let tid = thread_inner.res.as_ref().unwrap().tid;
        let mut user_sp = user_stack_position(tid).1;
        user_sp -= core::mem::size_of::<usize>();
        let ptr = translated_refmut(new_token, user_sp as *mut usize);
        *ptr = 0; // argv[argc] = NULL
        user_sp -= args.len() * core::mem::size_of::<usize>();
        let argv_base = user_sp;

        for (i, arg) in args.iter().enumerate() {
            // 将参数字符串写入用户栈
            user_sp -= arg.len() + 1;
            let mut p = user_sp;
            for c in arg.as_bytes() {
                *translated_refmut(new_token, p as *mut u8) = *c;
                p += 1;
            }
            *translated_refmut(new_token, p as *mut u8) = 0;
            // 将参数指针写入 argv[i]
            let ptr = translated_refmut(
                new_token,
                (argv_base + i * core::mem::size_of::<usize>()) as *mut usize,
            );
            *ptr = user_sp;
        }

        user_sp -= user_sp % core::mem::size_of::<usize>();

        let mut trap_cx = TrapContext::app_init_context(
            entry_point,
            user_sp,
            KERNEL_SPACE.exclusive_access().token(),
            thread.kernel_stack.get_top(),
            trap_handler as *const () as usize,
        );
        trap_cx.x[10] = args.len();
        trap_cx.x[11] = argv_base;
        *thread_inner.get_trap_cx() = trap_cx;
    }

    pub fn fork(self: &Arc<Self>) -> Arc<Self> {
        let mut parent_inner = self.inner.exclusive_access();
        assert_eq!(parent_inner.thread_count(), 1);
        let memory_set = MemorySet::from_another(&parent_inner.memory_set);
        let pid = pid::pid_alloc();
        let mut new_fd_table = Vec::new();
        for fd in &parent_inner.fd_table {
            if let Some(file) = fd {
                new_fd_table.push(Some(file.clone()));
            } else {
                new_fd_table.push(None);
            }
        }
        let child_pcb = Arc::new(Self {
            pid,
            inner: unsafe {
                SyncRefCell::new(ProcessControlBlockInner {
                    is_zombie: false,
                    memory_set,
                    parent: Some(Arc::downgrade(self)),
                    children: Vec::new(),
                    exit_code: 0,
                    fd_table: new_fd_table,
                    tasks: Vec::new(),
                    task_res_allocator: RecycleAllocator::new(),
                    mutex_list: Vec::new(),
                    sem_list: Vec::new(),
                    cond_list: Vec::new(),
                    signals: SignalFlags::empty(),
                    heap_bottom: parent_inner.heap_bottom,
                    program_brk: parent_inner.program_brk,
                    cwd: parent_inner.cwd.clone(),
                })
            },
        });
        parent_inner.children.push(child_pcb.clone());
        let thread = Arc::new(ThreadControlBlock::new(Arc::clone(&child_pcb), false));
        let mut child_inner = child_pcb.inner_exclusive_access();
        child_inner.tasks.push(Some(thread.clone()));
        drop(child_inner);
        let thread_inner = thread.inner_exclusive_access();
        let trap_cx = thread_inner.get_trap_cx();
        trap_cx.kernel_sp = thread.kernel_stack.get_top();
        drop(thread_inner);
        insert_into_pid2process(child_pcb.getpid(), child_pcb.clone());
        add_task(thread);
        child_pcb
    }

    pub fn getpid(&self) -> usize {
        self.pid.0
    }
}

pub struct ProcessControlBlockInner {
    pub is_zombie: bool,
    pub memory_set: MemorySet,
    pub parent: Option<Weak<ProcessControlBlock>>,
    pub children: Vec<Arc<ProcessControlBlock>>,
    pub exit_code: i32,
    pub fd_table: Vec<Option<Arc<dyn File>>>,
    pub tasks: Vec<Option<Arc<ThreadControlBlock>>>,
    pub task_res_allocator: RecycleAllocator,
    pub mutex_list: Vec<Option<Arc<dyn Mutex>>>,
    pub sem_list: Vec<Option<Arc<Semaphore>>>,
    pub cond_list: Vec<Option<Arc<Condvar>>>,
    pub signals: SignalFlags,
    // my custom
    pub heap_bottom: usize,
    pub program_brk: usize,
    pub cwd: String,
}

impl ProcessControlBlockInner {
    pub fn get_user_token(&self) -> usize {
        self.memory_set.token()
    }

    pub fn alloc_fd(&mut self) -> usize {
        if let Some(pos) = self.fd_table.iter().position(|fd| fd.is_none()) {
            pos
        } else {
            let pos = self.fd_table.len();
            self.fd_table.push(None);
            pos
        }
    }

    pub fn alloc_tid(&mut self) -> usize {
        self.task_res_allocator.alloc()
    }

    pub fn dealloc_tid(&mut self, tid: usize) {
        self.task_res_allocator.dealloc(tid)
    }

    pub fn thread_count(&self) -> usize {
        self.tasks.len()
    }

    pub fn get_task(&self, tid: usize) -> Arc<ThreadControlBlock> {
        self.tasks[tid].as_ref().unwrap().clone()
    }

    /// change the location of the program break. return None if failed.
    pub fn change_program_brk(&mut self, size: i32) -> KernelResult<usize> {
        let old_break = self.program_brk;
        let new_brk = self.program_brk as isize + size as isize;
        if new_brk < self.heap_bottom as isize {
            return Err(KernelError::ReleaseTooMuch);
        }
        let result = if size < 0 {
            self.memory_set
                .shrink_to(VirtAddr(self.heap_bottom), VirtAddr(new_brk as usize))
        } else {
            self.memory_set
                .append_to(VirtAddr(self.heap_bottom), VirtAddr(new_brk as usize))
        };
        match result {
            Ok(()) => {
                self.program_brk = new_brk as usize;
                Ok(old_break)
            }
            Err(e) => Err(e),
        }
    }
    pub fn mmap(&mut self, start: usize, end: usize, prot: usize) -> isize {
        self.memory_set.mmap(start, end, prot)
    }
    pub fn munmap(&mut self, start: usize, end: usize) -> isize {
        self.memory_set.munmap(start, end)
    }
}

pub struct ThreadControlBlockInner {
    pub res: Option<ThreadUserRes>,
    pub trap_cx_ppn: PhysPageNum,
    pub task_cx: TaskContext,
    pub task_status: TaskStatus,
    pub exit_code: Option<i32>,
}

#[derive(Copy, Clone, PartialEq)]
pub enum TaskStatus {
    Ready,
    Running,
    Blocked,
}

impl ThreadControlBlockInner {
    pub fn get_trap_cx(&self) -> &'static mut TrapContext {
        self.trap_cx_ppn.get_mut()
    }
}

/// ThreadControlBlock
pub struct ThreadControlBlock {
    pub process: Weak<ProcessControlBlock>,
    pub kernel_stack: KernelStack,
    inner: SyncRefCell<ThreadControlBlockInner>,
}

impl ThreadControlBlock {
    pub fn new(process: Arc<ProcessControlBlock>, alloc_user_res: bool) -> Self {
        let res = ThreadUserRes::new(Arc::clone(&process), alloc_user_res);
        debug!(
            "分配线程资源 TID = {}, trap_cx_ppn = {:#x}",
            res.trap_cx_ppn().0,
            res.trap_cx_ppn().0
        );
        let trap_cx_ppn = res.trap_cx_ppn();
        let kernel_stack = kstack_alloc();
        let kstack_top = kernel_stack.get_top();

        Self {
            process: Arc::downgrade(&process),
            kernel_stack,
            inner: unsafe {
                SyncRefCell::new(ThreadControlBlockInner {
                    res: Some(res),
                    trap_cx_ppn,
                    task_cx: TaskContext::goto_trap_return(kstack_top),
                    task_status: TaskStatus::Ready,
                    exit_code: None,
                })
            },
        }
    }

    pub fn inner_exclusive_access(&self) -> RefMut<'_, ThreadControlBlockInner> {
        self.inner.exclusive_access()
    }
}
