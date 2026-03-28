//! 任务控制块

use core::cell::RefMut;

use alloc::{
    string::String,
    sync::{Arc, Weak},
    vec,
    vec::Vec,
};

use crate::{
    config::{TRAP_CONTEXT, USER_STACK_TOP},
    error::{KernelError, KernelResult},
    fs::File,
    mem::{KERNEL_SPACE, MemorySet, PhysPageNum, VirtAddr, translated_refmut},
    sync::SyncRefCell,
    task::pid::{self, KernelStack, Pid},
    trap::{TrapContext, trap_handler},
};

use super::TaskContext;

pub struct TaskControlBlock {
    pub task_status: TaskStatus,
    pub task_cx: TaskContext,
    pub memory_set: MemorySet,
    pub trap_cx_ppn: PhysPageNum,
    pub heap_bottom: usize,
    pub program_brk: usize,
    pub parent: Option<Weak<ProcessControlBlock>>,
    pub children: Vec<Arc<ProcessControlBlock>>,
    pub exit_code: i32,
    pub fd_table: Vec<Option<Arc<dyn File>>>,
}

#[derive(Copy, Clone, PartialEq)]
pub enum TaskStatus {
    Ready,
    Running,
    Zombie,
}

impl TaskControlBlock {
    pub fn get_trap_cx(&self) -> &'static mut TrapContext {
        self.trap_cx_ppn.get_mut()
    }
    pub fn get_user_token(&self) -> usize {
        self.memory_set.token()
    }
    pub fn get_status(&self) -> TaskStatus {
        self.task_status
    }
    pub fn is_zombie(&self) -> bool {
        self.get_status() == TaskStatus::Zombie
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
    pub fn alloc_fd(&mut self) -> usize {
        if let Some(pos) = self.fd_table.iter().position(|fd| fd.is_none()) {
            pos
        } else {
            let pos = self.fd_table.len();
            self.fd_table.push(None);
            pos
        }
    }
}

/// ProcessControlBlock
pub struct ProcessControlBlock {
    pub pid: Pid,
    pub kernel_stack: KernelStack,
    inner: SyncRefCell<TaskControlBlock>,
}

impl ProcessControlBlock {
    pub fn new(elf_data: &[u8]) -> Self {
        let (memory_set, heap_bottom, entry_point) = MemorySet::from_elf(elf_data);
        let trap_cx_ppn = memory_set
            .translate(VirtAddr(TRAP_CONTEXT).into())
            .unwrap()
            .ppn();
        let pid = pid::pid_alloc();
        let kernel_stack = KernelStack::new(&pid);
        let kernel_stack_top = kernel_stack.get_top();
        let pcb = Self {
            pid,
            kernel_stack,
            inner: unsafe {
                SyncRefCell::new(TaskControlBlock {
                    task_status: TaskStatus::Ready,
                    task_cx: TaskContext::goto_trap_return(kernel_stack_top),
                    memory_set,
                    trap_cx_ppn,
                    heap_bottom,
                    program_brk: heap_bottom,
                    parent: None,
                    children: Vec::new(),
                    exit_code: 0,
                    fd_table: vec![
                        Some(Arc::new(crate::fs::stdio::Stdin)),
                        Some(Arc::new(crate::fs::stdio::Stdout)),
                        Some(Arc::new(crate::fs::stdio::Stderr)),
                    ],
                })
            },
        };
        let trap_cx = pcb.inner.exclusive_access().get_trap_cx();
        *trap_cx = TrapContext::app_init_context(
            entry_point,
            USER_STACK_TOP,
            KERNEL_SPACE.exclusive_access().token(),
            kernel_stack_top,
            trap_handler as *const () as usize,
        );
        pcb
    }

    pub fn inner_exclusive_access(&self) -> RefMut<'_, TaskControlBlock> {
        self.inner.exclusive_access()
    }
    pub fn get_pid(&self) -> usize {
        self.pid.0
    }
    pub fn exec(&self, elf_data: &[u8], args: Vec<String>) {
        let (memory_set, heap_bottom, entry_point) = MemorySet::from_elf(elf_data);
        let trap_cx_ppn = memory_set
            .translate(VirtAddr(TRAP_CONTEXT).into())
            .unwrap()
            .ppn();
        let mut user_sp = USER_STACK_TOP;
        user_sp -= core::mem::size_of::<usize>();
        let ptr = translated_refmut(memory_set.token(), user_sp as *mut usize);
        *ptr = 0; // argv[argc] = NULL
        user_sp -= args.len() * core::mem::size_of::<usize>();
        let argv_base = user_sp;

        for (i, arg) in args.iter().enumerate() {
            // 将参数字符串写入用户栈
            user_sp -= arg.len() + 1;
            let mut p = user_sp;
            for c in arg.as_bytes() {
                *translated_refmut(memory_set.token(), p as *mut u8) = *c;
                p += 1;
            }
            *translated_refmut(memory_set.token(), p as *mut u8) = 0;
            // 将参数指针写入 argv[i]
            let ptr = translated_refmut(
                memory_set.token(),
                (argv_base + i * core::mem::size_of::<usize>()) as *mut usize,
            );
            *ptr = user_sp;
        }

        user_sp -= user_sp % core::mem::size_of::<usize>();

        let mut inner = self.inner.exclusive_access();
        inner.memory_set = memory_set;
        inner.trap_cx_ppn = trap_cx_ppn;
        inner.heap_bottom = heap_bottom;
        inner.program_brk = heap_bottom;
        let mut trap_cx = TrapContext::app_init_context(
            entry_point,
            user_sp,
            KERNEL_SPACE.exclusive_access().token(),
            self.kernel_stack.get_top(),
            trap_handler as *const () as usize,
        );
        trap_cx.x[10] = args.len();
        trap_cx.x[11] = argv_base;
        *inner.get_trap_cx() = trap_cx;
    }
    pub fn fork(self: &Arc<Self>) -> Arc<Self> {
        let mut parent_inner = self.inner.exclusive_access();
        let memory_set = MemorySet::from_another(&parent_inner.memory_set);
        let trap_cx_ppn = memory_set
            .translate(VirtAddr(TRAP_CONTEXT).into())
            .unwrap()
            .ppn();
        let pid = pid::pid_alloc();
        let kernel_stack = KernelStack::new(&pid);
        let kernel_stack_top = kernel_stack.get_top();
        let child_pcb = Arc::new(Self {
            pid,
            kernel_stack,
            inner: unsafe {
                SyncRefCell::new(TaskControlBlock {
                    task_status: TaskStatus::Ready,
                    task_cx: TaskContext::goto_trap_return(kernel_stack_top),
                    memory_set,
                    trap_cx_ppn,
                    heap_bottom: parent_inner.heap_bottom,
                    program_brk: parent_inner.program_brk,
                    parent: Some(Arc::downgrade(self)),
                    children: Vec::new(),
                    exit_code: 0,
                    fd_table: parent_inner.fd_table.clone(),
                })
            },
        });
        parent_inner.children.push(child_pcb.clone());
        let trap_cx = child_pcb.inner.exclusive_access().get_trap_cx();
        trap_cx.kernel_sp = kernel_stack_top;
        child_pcb
    }
}
