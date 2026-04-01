//! syscalls 的实现
//! 当用户空间使用 `ecall` 指令发起系统调用时，
//! 处理器会引发一个 '来自 U 模式的环境调用' 异常，
//! 这在 [`crate::trap::trap_handler`] 的某个 case 中被处理。
//! 为了清晰起见，每个系统调用都被实现为一个独立的函数，
//! 命名为 `sys_` 加上系统调用的名称。
//! 你可以在子模块中找到这样的函数，你也应该以这种方式实现系统调用。

const SYSCALL_GETCWD: usize = 17;
const SYSCALL_DUP: usize = 24;
const SYSCALL_UNLINKAT: usize = 35;
const SYSCALL_CHDIR: usize = 49;
const SYSCALL_OPENAT: usize = 56;
const SYSCALL_CLOSE: usize = 57;
const SYSCALL_PIPE: usize = 59;
const SYSCALL_READ: usize = 63;
const SYSCALL_WRITE: usize = 64;
const SYSCALL_EXIT: usize = 93;
const SYSCALL_YIELD: usize = 124;
const SYSCALL_GET_TIME: usize = 169;
const SYSCALL_SBRK: usize = 214;
const SYSCALL_MMAP: usize = 222;
const SYSCALL_MUNMAP: usize = 215;
const SYSCALL_FORK: usize = 220;
const SYSCALL_EXEC: usize = 221;
const SYSCALL_WAITPID: usize = 260;
const SYSCALL_GETTID: usize = 178;
const SYSCALL_THREAD_CREATE: usize = 460;
const SYSCALL_WAITTID: usize = 462;
const SYSCALL_MUTEX_CREATE: usize = 463;
const SYSCALL_MUTEX_LOCK: usize = 464;
const SYSCALL_MUTEX_UNLOCK: usize = 466;
const SYSCALL_SEMAPHORE_CREATE: usize = 467;
const SYSCALL_SEMAPHORE_UP: usize = 468;
const SYSCALL_SEMAPHORE_DOWN: usize = 469;

mod fs;
mod mem;
mod process;
mod sync;
mod thread;

use fs::*;
use log::trace;
use mem::*;
use process::*;
use sync::*;
use thread::*;

/// 系统调用的入口函数
pub fn syscall(syscall_id: usize, args: [usize; 3]) -> isize {
    trace!("syscall: id={}, args={:?}", syscall_id, args);
    match syscall_id {
        SYSCALL_GETCWD => sys_getcwd(args[0] as *mut u8, args[1]),
        SYSCALL_DUP => sys_dup(args[0]),
        SYSCALL_UNLINKAT => sys_unlink(args[0] as *const u8),
        SYSCALL_CHDIR => sys_chdir(args[0] as *const u8),
        SYSCALL_OPENAT => sys_open(args[0] as *const u8, args[1] as u32),
        SYSCALL_CLOSE => sys_close(args[0]),
        SYSCALL_PIPE => sys_pipe(args[0] as *mut usize),
        SYSCALL_READ => sys_read(args[0], args[1] as *mut u8, args[2]),
        SYSCALL_WRITE => sys_write(args[0], args[1] as *const u8, args[2]),
        SYSCALL_EXIT => sys_exit(args[0] as i32),
        SYSCALL_YIELD => sys_yield(),
        SYSCALL_GET_TIME => sys_get_time(),
        SYSCALL_SBRK => sys_sbrk(args[0] as i32),
        SYSCALL_MMAP => sys_mmap(args[0], args[1], args[2]),
        SYSCALL_MUNMAP => sys_munmap(args[0], args[1]),
        SYSCALL_FORK => sys_fork(),
        SYSCALL_EXEC => sys_exec(args[0] as *const u8, args[1] as *const usize),
        SYSCALL_WAITPID => sys_waitpid(args[0] as isize, args[1] as *mut i32),
        SYSCALL_THREAD_CREATE => sys_thread_create(args[0], args[1]),
        SYSCALL_WAITTID => sys_waittid(args[0]) as isize,
        SYSCALL_GETTID => sys_gettid(),
        SYSCALL_MUTEX_CREATE => sys_mutex_create(args[0] != 0),
        SYSCALL_MUTEX_LOCK => sys_mutex_lock(args[0]),
        SYSCALL_MUTEX_UNLOCK => sys_mutex_unlock(args[0]),
        SYSCALL_SEMAPHORE_CREATE => sys_semaphore_create(args[0]),
        SYSCALL_SEMAPHORE_UP => sys_semaphore_up(args[0]),
        SYSCALL_SEMAPHORE_DOWN => sys_semaphore_down(args[0]),

        _ => panic!("Unsupported syscall_id: {}", syscall_id),
    }
}
