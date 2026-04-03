//! 进程相关的系统调用
use alloc::sync::Arc;
use alloc::vec::Vec;
use log::{info, warn};

use crate::fs::inode::{OpenFlags, chdir_path, open_file};
use crate::mem::{UserBuffer, try_translated_ref, try_translated_refmut, try_translated_str};
use crate::sbi::shutdown;
use crate::task::{
    INITPROCESS, SignalFlags, change_program_brk, current_process, current_task,
    current_user_token, exit_current_and_run_next, pid2process, suspend_current_and_run_next,
};
use crate::timer::get_time_ms;

/// 系统调用：退出当前应用并运行下一个应用
pub fn sys_exit(exit_code: i32) -> ! {
    let pid = current_process().getpid();
    let tid = current_task()
        .inner_exclusive_access()
        .res
        .as_ref()
        .map(|res| res.tid)
        .unwrap_or(usize::MAX);
    info!(
        "退出线程: pid[{}] tid[{}] exit_code[{}]",
        pid, tid, exit_code
    );
    exit_current_and_run_next(exit_code);
    panic!("Unreachable in sys_exit!");
}

/// 系统调用：让出 CPU 给其他应用
pub fn sys_yield() -> isize {
    let process = current_process();
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
    let current_process = current_process();
    let new_process = current_process.fork();
    let new_pid = new_process.pid.0;
    let new_process_inner = new_process.inner_exclusive_access();
    let Some(thread) = new_process_inner
        .tasks
        .first()
        .and_then(|thread| thread.as_ref())
    else {
        warn!("fork 后子进程缺少主线程");
        return -1;
    };
    let trap_cx = thread.inner_exclusive_access().get_trap_cx();
    trap_cx.x[10] = 0; // 子进程 fork 的返回值为 0
    new_pid as isize
}

pub fn sys_exec(path: *const u8, mut args: *const usize) -> isize {
    let token = current_user_token();
    let Some(path) = try_translated_str(token, path) else {
        return -1;
    };
    let mut args_vec = Vec::new();
    if !args.is_null() {
        loop {
            let Some(arg_ptr_ref) = try_translated_ref(token, args) else {
                return -1;
            };
            let arg_ptr = *arg_ptr_ref;
            if arg_ptr == 0 {
                break;
            }
            let Some(arg) = try_translated_str(token, arg_ptr as *const u8) else {
                return -1;
            };
            args_vec.push(arg);
            args = unsafe { args.add(1) };
        }
    }
    if args_vec.is_empty() {
        args_vec.push(path.clone());
    }
    info!("exec path: {}, args: {:?}", path, args_vec);
    if let Some(app_inode) = open_file(path.as_str(), OpenFlags::RDONLY) {
        let all_data = app_inode.read_all();
        let process = current_process();
        let argc = args_vec.len();
        process.exec(&all_data, args_vec);
        argc as isize
    } else {
        -1
    }
}

pub fn sys_waitpid(pid: isize, exit_code_ptr: *mut i32) -> isize {
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    if !process_inner
        .children
        .iter()
        .any(|p| pid == -1 || pid as usize == p.getpid())
    {
        return -1; // 子进程
    }
    let pair = process_inner.children.iter().enumerate().find(|(_, p)| {
        p.inner_exclusive_access().is_zombie && (pid == -1 || pid as usize == p.getpid())
    });
    if let Some((idx, _)) = pair {
        let child = process_inner.children.remove(idx);
        let pid = child.getpid();
        let exit_code = child.inner_exclusive_access().exit_code;
        let Some(exit_code_ref) =
            try_translated_refmut(process_inner.memory_set.token(), exit_code_ptr)
        else {
            return -1;
        };
        *exit_code_ref = exit_code;
        pid as isize
    } else {
        -2
    }
}

pub fn sys_getcwd(buf: *mut u8, len: usize) -> isize {
    if buf.is_null() || len == 0 {
        return -1;
    }
    let process = current_process();
    let cwd = process.inner_exclusive_access().cwd.clone();
    let bytes = cwd.as_bytes();
    if bytes.len() + 1 > len {
        return -1;
    }

    let token = current_user_token();
    let Some(user_buf) = UserBuffer::try_from_raw_parts(token, buf as *const u8, len) else {
        return -1;
    };
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
    let Some(path) = try_translated_str(token, path) else {
        return -1;
    };
    if let Some(new_cwd) = chdir_path(path.as_str()) {
        let process = current_process();
        process.inner_exclusive_access().cwd = new_cwd;
        0
    } else {
        -1
    }
}

pub fn sys_kill(pid: usize, signal: u32) -> isize {
    if let Some(process) = pid2process(pid)
        && let Some(flag) = SignalFlags::from_bits(signal)
    {
        process.inner_exclusive_access().signals |= flag;
        0
    } else {
        -1
    }
}
