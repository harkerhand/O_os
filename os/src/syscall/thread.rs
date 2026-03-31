use crate::{
    mem::KERNEL_SPACE,
    task::{ThreadControlBlock, add_task, current_task},
    trap::{TrapContext, trap_handler},
};
use alloc::sync::Arc;
use log::debug;
pub fn sys_thread_create(entry: usize, arg: usize) -> isize {
    let pid = current_task().unwrap().process.upgrade().unwrap().getpid();
    let tid = current_task()
        .unwrap()
        .inner_exclusive_access()
        .res
        .as_ref()
        .unwrap()
        .tid;
    debug!("创建线程: pid[{}] tid[{}]", pid, tid);
    let task = current_task().unwrap();
    let process = task.process.upgrade().unwrap();
    // create a new thread
    let new_task = Arc::new(ThreadControlBlock::new(Arc::clone(&process), true));
    // add new task to scheduler
    add_task(Arc::clone(&new_task));
    let new_task_inner = new_task.inner_exclusive_access();
    let new_task_res = new_task_inner.res.as_ref().unwrap();
    let new_task_tid = new_task_res.tid;
    let mut process_inner = process.inner_exclusive_access();
    // add new thread to current process
    let tasks = &mut process_inner.tasks;
    while tasks.len() < new_task_tid + 1 {
        tasks.push(None);
    }
    tasks[new_task_tid] = Some(Arc::clone(&new_task));
    let new_task_trap_cx = new_task_inner.get_trap_cx();
    *new_task_trap_cx = TrapContext::app_init_context(
        entry,
        new_task_res.ustack_top(),
        KERNEL_SPACE.exclusive_access().token(),
        new_task.kernel_stack.get_top(),
        trap_handler as *const () as usize,
    );
    new_task_trap_cx.x[10] = arg;
    new_task_tid as isize
}
pub fn sys_gettid() -> isize {
    let pid = current_task().unwrap().process.upgrade().unwrap().getpid();
    let tid = current_task()
        .unwrap()
        .inner_exclusive_access()
        .res
        .as_ref()
        .unwrap()
        .tid;
    debug!("获取线程 ID: pid[{}] tid[{}]", pid, tid);
    tid as isize
}

/// wait for a thread to exit syscall
///
/// thread does not exist, return -1
/// thread has not exited yet, return -2
/// otherwise, return thread's exit code
pub fn sys_waittid(tid: usize) -> i32 {
    let pid = current_task().unwrap().process.upgrade().unwrap().getpid();
    debug!("等待线程退出: pid[{}] tid[{}]", pid, tid);
    let task = current_task().unwrap();
    let process = task.process.upgrade().unwrap();
    let task_inner = task.inner_exclusive_access();
    let mut process_inner = process.inner_exclusive_access();
    if tid >= process_inner.tasks.len() {
        return -1;
    }
    // a thread cannot wait for itself
    if task_inner.res.as_ref().unwrap().tid == tid {
        return -1;
    }
    let mut exit_code: Option<i32> = None;
    let waited_task = process_inner.tasks[tid].as_ref();
    if let Some(waited_task) = waited_task {
        if let Some(waited_exit_code) = waited_task.inner_exclusive_access().exit_code {
            exit_code = Some(waited_exit_code);
        }
    } else {
        // waited thread does not exist
        return -1;
    }
    if let Some(exit_code) = exit_code {
        // 先把线程从任务表摘下来，避免持有 process_inner 锁时触发线程资源 Drop。
        // ThreadUserRes::drop 会再次借用 process.inner，若在锁内直接置 None 会触发二次借用 panic。
        let recycled_thread = process_inner.tasks[tid].take();
        drop(process_inner);
        drop(task_inner);
        drop(recycled_thread);
        exit_code
    } else {
        // waited thread has not exited
        -2
    }
}
