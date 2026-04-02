//! 文件系统相关的系统调用

use log::info;

use crate::{
    fs::inode::{OpenFlags, open_file, unlink_file},
    mem::{UserBuffer, try_translated_refmut, try_translated_str},
    task::{current_process, current_user_token},
};

/// 系统调用：打开文件，返回文件描述符
pub fn sys_open(path: *const u8, flags: u32) -> isize {
    let process = current_process();
    let token = current_user_token();
    let Some(path) = try_translated_str(token, path) else {
        return -1;
    };
    let Some(flags) = OpenFlags::from_bits(flags) else {
        return -1;
    };
    if let Some(inode) = open_file(path.as_str(), flags) {
        let mut inner = process.inner_exclusive_access();
        let fd = inner.alloc_fd();
        inner.fd_table[fd] = Some(inode);
        fd as isize
    } else {
        -1
    }
}

/// 系统调用：关闭文件描述符
pub fn sys_close(fd: usize) -> isize {
    let process = current_process();
    let mut inner = process.inner_exclusive_access();
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
    let current_pcb = current_process();
    let current_tcb = current_pcb.inner_exclusive_access();
    if fd < current_tcb.fd_table.len()
        && let Some(file) = &current_tcb.fd_table[fd]
    {
        let file = file.clone();
        if !file.writeable() {
            return -1;
        }
        drop(current_tcb);
        let Some(user_buf) = UserBuffer::try_from_raw_parts(token, buf, len) else {
            return -1;
        };
        file.write(user_buf)
    } else {
        -1
    }
}

pub fn sys_read(fd: usize, buf: *mut u8, len: usize) -> isize {
    let token = current_user_token();
    let current_pcb = current_process();
    let current_tcb = current_pcb.inner_exclusive_access();
    if fd < current_tcb.fd_table.len()
        && let Some(file) = &current_tcb.fd_table[fd]
    {
        let file = file.clone();
        if !file.readable() {
            return -1;
        }
        drop(current_tcb);
        let Some(user_buf) = UserBuffer::try_from_raw_parts(token, buf, len) else {
            return -1;
        };
        file.read(user_buf)
    } else {
        -1
    }
}

pub fn sys_unlink(path: *const u8) -> isize {
    let token = current_user_token();
    let Some(path) = try_translated_str(token, path) else {
        return -1;
    };
    if unlink_file(path.as_str()) { 0 } else { -1 }
}

pub fn sys_pipe(pipe: *mut usize) -> isize {
    let process = current_process();
    let token = current_user_token();
    let mut inner = process.inner_exclusive_access();
    let (read_end, write_end) = crate::fs::pipe::make_pipe();
    let read_fd = inner.alloc_fd();
    inner.fd_table[read_fd] = Some(read_end);
    let write_fd = inner.alloc_fd();
    inner.fd_table[write_fd] = Some(write_end);
    let Some(read_fd_ptr) = try_translated_refmut(token, pipe) else {
        return -1;
    };
    let Some(write_fd_ptr) = try_translated_refmut(token, unsafe { pipe.add(1) }) else {
        return -1;
    };
    *read_fd_ptr = read_fd;
    *write_fd_ptr = write_fd;
    info!("新建管道，读端 fd = {}, 写端 fd = {}", read_fd, write_fd);
    0
}

pub fn sys_dup(fd: usize) -> isize {
    let process = current_process();
    let mut inner = process.inner_exclusive_access();
    if fd < inner.fd_table.len()
        && let Some(file) = inner.fd_table[fd].clone()
    {
        let new_fd = inner.alloc_fd();
        inner.fd_table[new_fd] = Some(file);
        new_fd as isize
    } else {
        -1
    }
}
