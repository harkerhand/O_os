use crate::sync::{MutexBlocking, MutexSpin, Semaphore};
use crate::task::current_process;
use alloc::sync::Arc;
/// mutex create syscall
pub fn sys_mutex_create(blocking: bool) -> isize {
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();

    if let Some(id) = process_inner
        .mutex_list
        .iter()
        .enumerate()
        .find(|(_, item)| item.is_none())
        .map(|(id, _)| id)
    {
        process_inner.mutex_list[id] = if !blocking {
            Some(Arc::new(MutexSpin::new()))
        } else {
            Some(Arc::new(MutexBlocking::new()))
        };
        id as isize
    } else {
        process_inner
            .mutex_list
            .push(Some(Arc::new(MutexSpin::new())));
        process_inner.mutex_list.len() as isize - 1
    }
}
pub fn sys_mutex_lock(mutex_id: usize) -> isize {
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let mutex = Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap());
    drop(process_inner);
    drop(process);
    mutex.lock();
    0
}
pub fn sys_mutex_unlock(mutex_id: usize) -> isize {
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let mutex = Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap());
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
    let sem = Arc::clone(process_inner.sem_list[sem_id].as_ref().unwrap());
    drop(process_inner);
    drop(process);
    sem.up();
    0
}

pub fn sys_semaphore_down(sem_id: usize) -> isize {
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let sem = Arc::clone(process_inner.sem_list[sem_id].as_ref().unwrap());
    drop(process_inner);
    drop(process);
    sem.down();
    0
}
