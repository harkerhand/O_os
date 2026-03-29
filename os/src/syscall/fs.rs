//! 文件系统相关的系统调用

use crate::{
    fs::inode::{OpenFlags, open_file, unlink_file},
    mem::{UserBuffer, translated_str},
    task::{current_task, current_user_token},
};

/// 系统调用：打开文件，返回文件描述符
pub fn sys_open(path: *const u8, flags: u32) -> isize {
    let task = current_task().unwrap();
    let token = current_user_token();
    let path = translated_str(token, path);
    if let Some(inode) = open_file(path.as_str(), OpenFlags::from_bits(flags).unwrap()) {
        let mut inner = task.inner_exclusive_access();
        let fd = inner.alloc_fd();
        inner.fd_table[fd] = Some(inode);
        fd as isize
    } else {
        -1
    }
}

/// 系统调用：关闭文件描述符
pub fn sys_close(fd: usize) -> isize {
    let task = current_task().unwrap();
    let mut inner = task.inner_exclusive_access();
    if fd >= inner.fd_table.len() {
        return -1;
    }
    if inner.fd_table[fd].is_none() {
        return -1;
    }
    inner.fd_table[fd].take();
    0
}

/// 向文件描述符写入数据
pub fn sys_write(fd: usize, buf: *const u8, len: usize) -> isize {
    let token = current_user_token();
    let current_pcb = current_task().unwrap();
    let current_tcb = current_pcb.inner_exclusive_access();
    if fd < current_tcb.fd_table.len()
        && let Some(file) = &current_tcb.fd_table[fd]
    {
        let file = file.clone();
        assert!(file.writeable());
        drop(current_tcb);
        file.write(UserBuffer::from_raw_parts(token, buf, len))
    } else {
        -1
    }
}

pub fn sys_read(fd: usize, buf: *mut u8, len: usize) -> isize {
    let token = current_user_token();
    let current_pcb = current_task().unwrap();
    let current_tcb = current_pcb.inner_exclusive_access();
    if fd < current_tcb.fd_table.len()
        && let Some(file) = &current_tcb.fd_table[fd]
    {
        let file = file.clone();
        assert!(file.readable());
        drop(current_tcb);
        file.read(UserBuffer::from_raw_parts(token, buf, len))
    } else {
        -1
    }
}

pub fn sys_unlink(path: *const u8) -> isize {
    let token = current_user_token();
    let path = translated_str(token, path);
    if unlink_file(path.as_str()) { 0 } else { -1 }
}
