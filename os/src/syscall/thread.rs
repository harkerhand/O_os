use crate::{
    mem::KERNEL_SPACE,
    task::{ThreadControlBlock, add_task, current_process, current_task},
    trap::{TrapContext, trap_handler},
};
use alloc::sync::Arc;
use log::{debug, warn};
pub fn sys_thread_create(entry: usize, arg: usize) -> isize {
    let task = current_task();
    let process = current_process();
    let Some(tid) = task
        .inner_exclusive_access()
        .res
        .as_ref()
        .map(|res| res.tid)
    else {
        warn!("线程创建失败: 当前线程资源缺失");
        return -1;
    };
    let pid = process.getpid();
    debug!("创建线程: pid[{}] tid[{}]", pid, tid);
    // create a new thread
    let new_task = Arc::new(ThreadControlBlock::new(Arc::clone(&process), true));
    // add new task to scheduler
    add_task(Arc::clone(&new_task));
    let new_task_inner = new_task.inner_exclusive_access();
    let Some(new_task_res) = new_task_inner.res.as_ref() else {
        warn!("线程创建失败: 新线程资源缺失");
        return -1;
    };
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
    let task = current_task();
    let process = current_process();
    let Some(tid) = task
        .inner_exclusive_access()
        .res
        .as_ref()
        .map(|res| res.tid)
    else {
        return -1;
    };
    let pid = process.getpid();
    debug!("获取线程 ID: pid[{}] tid[{}]", pid, tid);
    tid as isize
}

/// wait for a thread to exit syscall
///
/// thread does not exist, return -1
/// thread has not exited yet, return -2
/// otherwise, return thread's exit code
pub fn sys_waittid(tid: usize) -> i32 {
    let task = current_task();
    let process = current_process();
    let Some(current_tid) = task
        .inner_exclusive_access()
        .res
        .as_ref()
        .map(|res| res.tid)
    else {
        warn!("等待线程退出失败: 当前线程资源缺失");
        return -1;
    };
    let pid = process.getpid();
    debug!("等待线程退出: pid[{}] tid[{}]", pid, tid);
    let mut process_inner = process.inner_exclusive_access();
    if tid >= process_inner.tasks.len() {
        return -1;
    }
    // a thread cannot wait for itself
    if current_tid == tid {
        return -1;
    }
    let mut exit_code: Option<i32> = None;
    if let Some(waited_task) = process_inner.tasks.get(tid).and_then(|task| task.as_ref()) {
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
        drop(recycled_thread);
        exit_code
    } else {
        // waited thread has not exited
        -2
    }
}
