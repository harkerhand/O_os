//! 进程相关的系统调用
use alloc::sync::Arc;
use alloc::vec::Vec;
use log::{info, warn};

use crate::fs::inode::{OpenFlags, chdir_path, open_file};
use crate::mem::{UserBuffer, translated_ref, translated_refmut, translated_str};
use crate::sbi::shutdown;
use crate::task::{
    INITPROCESS, add_task, change_program_brk, current_task, current_user_token,
    exit_current_and_run_next, suspend_current_and_run_next,
};
use crate::timer::get_time_ms;

/// 系统调用：退出当前应用并运行下一个应用
pub fn sys_exit(exit_code: i32) -> ! {
    info!("应用退出，退出码为 {}", exit_code);
    exit_current_and_run_next(exit_code);
    panic!("Unreachable in sys_exit!");
}

/// 系统调用：让出 CPU 给其他应用
pub fn sys_yield() -> isize {
    let process = current_task().unwrap();
    if Arc::ptr_eq(&process, &INITPROCESS) && process.inner_exclusive_access().children.is_empty() {
        info!("initproc has no children, shutting down.");
        shutdown();
    }
    suspend_current_and_run_next();
    0
}

/// 系统调用：获取当前时间（单位：毫秒）
pub fn sys_get_time() -> isize {
    get_time_ms() as isize
}

/// 系统调用：调整程序的 break（即数据段的末尾）的位置，返回旧的 break 地址
pub fn sys_sbrk(size: i32) -> isize {
    match change_program_brk(size) {
        Ok(old_brk) => old_brk as isize,
        Err(e) => {
            warn!("调整程序 break 失败: {:?}", e);
            -1
        }
    }
}

pub fn sys_fork() -> isize {
    let current_process = current_task().unwrap();
    let new_process = current_process.fork();
    let new_pid = new_process.pid.0;
    let trap_cx = new_process.inner_exclusive_access().get_trap_cx();
    trap_cx.x[10] = 0; // 子进程 fork 的返回值为 0
    add_task(new_process);
    new_pid as isize
}

pub fn sys_exec(path: *const u8, mut args: *const usize) -> isize {
    let token = current_user_token();
    let path = translated_str(token, path);
    let mut args_vec = Vec::new();
    if !args.is_null() {
        loop {
            let arg_ptr = *translated_ref(token, args);
            if arg_ptr == 0 {
                break;
            }
            args_vec.push(translated_str(token, arg_ptr as *const u8));
            args = unsafe { args.add(1) };
        }
    }
    if args_vec.is_empty() {
        args_vec.push(path.clone());
    }
    info!("exec path: {}, args: {:?}", path, args_vec);
    if let Some(app_inode) = open_file(path.as_str(), OpenFlags::RDONLY) {
        let all_data = app_inode.read_all();
        let task = current_task().unwrap();
        let argc = args_vec.len();
        task.exec(&all_data, args_vec);
        argc as isize
    } else {
        -1
    }
}

pub fn sys_waitpid(pid: isize, exit_code_ptr: *mut i32) -> isize {
    let process = current_task().unwrap();
    let mut current_tcb = process.inner_exclusive_access();
    if !current_tcb
        .children
        .iter()
        .any(|p| pid == -1 || pid as usize == p.get_pid())
    {
        return -1; // 子进程
    }
    let pair = current_tcb.children.iter().enumerate().find(|(_, p)| {
        p.inner_exclusive_access().is_zombie() && (pid == -1 || pid as usize == p.get_pid())
    });
    if let Some((idx, _)) = pair {
        let child = current_tcb.children.remove(idx);
        assert_eq!(
            Arc::strong_count(&child),
            1,
            "Child process should have no other references"
        );
        let pid = child.get_pid();
        let exit_code = child.inner_exclusive_access().exit_code;
        *translated_refmut(current_tcb.memory_set.token(), exit_code_ptr) = exit_code;
        pid as isize
    } else {
        -2
    }
}

pub fn sys_getcwd(buf: *mut u8, len: usize) -> isize {
    if buf.is_null() || len == 0 {
        return -1;
    }
    let task = current_task().unwrap();
    let cwd = task.inner_exclusive_access().cwd.clone();
    let bytes = cwd.as_bytes();
    if bytes.len() + 1 > len {
        return -1;
    }

    let token = current_user_token();
    let user_buf = UserBuffer::from_raw_parts(token, buf as *const u8, len);
    let mut idx = 0usize;
    for chunk in user_buf.buf {
        for byte in chunk.iter_mut() {
            if idx < bytes.len() {
                *byte = bytes[idx];
            } else if idx == bytes.len() {
                *byte = 0;
                return (bytes.len() + 1) as isize;
            } else {
                return (bytes.len() + 1) as isize;
            }
            idx += 1;
        }
    }
    -1
}

pub fn sys_chdir(path: *const u8) -> isize {
    if path.is_null() {
        return -1;
    }
    let token = current_user_token();
    let path = translated_str(token, path);
    if let Some(new_cwd) = chdir_path(path.as_str()) {
        let task = current_task().unwrap();
        task.inner_exclusive_access().cwd = new_cwd;
        0
    } else {
        -1
    }
}
