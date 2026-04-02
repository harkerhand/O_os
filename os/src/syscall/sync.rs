use crate::sync::{Condvar, Mutex, MutexBlocking, MutexSpin, Semaphore};
use crate::task::{current_process, current_task, mark_current_blocked, schedule};
use crate::timer::{add_timer, get_time_ms};
use alloc::sync::Arc;

pub fn sys_sleep(ms: usize) -> isize {
    let expire_ms = get_time_ms() + ms;
    let thread = current_task();
    let task_cx_ptr = mark_current_blocked();
    add_timer(expire_ms, thread);
    schedule(task_cx_ptr);
    0
}

/// mutex create syscall
pub fn sys_mutex_create(blocking: bool) -> isize {
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();

    let mutex: Arc<dyn Mutex> = if !blocking {
        Arc::new(MutexSpin::new())
    } else {
        Arc::new(MutexBlocking::new())
    };
    if let Some(id) = process_inner
        .mutex_list
        .iter()
        .enumerate()
        .find(|(_, item)| item.is_none())
        .map(|(id, _)| id)
    {
        process_inner.mutex_list[id] = Some(mutex);
        id as isize
    } else {
        process_inner.mutex_list.push(Some(mutex));
        process_inner.mutex_list.len() as isize - 1
    }
}
pub fn sys_mutex_lock(mutex_id: usize) -> isize {
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let Some(Some(mutex)) = process_inner.mutex_list.get(mutex_id) else {
        return -1;
    };
    let mutex = Arc::clone(mutex);
    drop(process_inner);
    drop(process);
    mutex.lock();
    0
}
pub fn sys_mutex_unlock(mutex_id: usize) -> isize {
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let Some(Some(mutex)) = process_inner.mutex_list.get(mutex_id) else {
        return -1;
    };
    let mutex = Arc::clone(mutex);
    drop(process_inner);
    drop(process);
    mutex.unlock();
    0
}

pub fn sys_semaphore_create(res_count: usize) -> isize {
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();

    if let Some(id) = process_inner
        .sem_list
        .iter()
        .enumerate()
        .find(|(_, item)| item.is_none())
        .map(|(id, _)| id)
    {
        process_inner.sem_list[id] = Some(Arc::new(Semaphore::new(res_count as isize)));
        id as isize
    } else {
        process_inner
            .sem_list
            .push(Some(Arc::new(Semaphore::new(res_count as isize))));
        process_inner.sem_list.len() as isize - 1
    }
}

pub fn sys_semaphore_up(sem_id: usize) -> isize {
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let Some(Some(sem)) = process_inner.sem_list.get(sem_id) else {
        return -1;
    };
    let sem = Arc::clone(sem);
    drop(process_inner);
    drop(process);
    sem.up();
    0
}

pub fn sys_semaphore_down(sem_id: usize) -> isize {
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let Some(Some(sem)) = process_inner.sem_list.get(sem_id) else {
        return -1;
    };
    let sem = Arc::clone(sem);
    drop(process_inner);
    drop(process);
    sem.down();
    0
}

pub fn sys_condvar_create() -> isize {
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    let id = if let Some(id) = process_inner
        .cond_list
        .iter()
        .enumerate()
        .find(|(_, item)| item.is_none())
        .map(|(id, _)| id)
    {
        process_inner.cond_list[id] = Some(Arc::new(Condvar::new()));
        id
    } else {
        process_inner.cond_list.push(Some(Arc::new(Condvar::new())));
        process_inner.cond_list.len() - 1
    };
    id as isize
}
/// condvar signal syscall
pub fn sys_condvar_signal(condvar_id: usize) -> isize {
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let Some(Some(condvar)) = process_inner.cond_list.get(condvar_id) else {
        return -1;
    };
    let condvar = Arc::clone(condvar);
    drop(process_inner);
    condvar.signal();
    0
}
/// condvar wait syscall
pub fn sys_condvar_wait(condvar_id: usize, mutex_id: usize) -> isize {
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let Some(Some(condvar)) = process_inner.cond_list.get(condvar_id) else {
        return -1;
    };
    let Some(Some(mutex)) = process_inner.mutex_list.get(mutex_id) else {
        return -1;
    };
    let condvar = Arc::clone(condvar);
    let mutex = Arc::clone(mutex);
    drop(process_inner);
    condvar.wait(mutex);
    0
}
