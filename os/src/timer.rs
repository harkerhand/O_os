//! RISC-V 时间相关

use crate::config::CLOCK_FREQ;
use crate::fs::stdio::poll_stdin;
use crate::sbi::set_timer;
use crate::sync::SyncRefCell;
use crate::task::{ThreadControlBlock, wakeup_task};
use alloc::collections::BinaryHeap;
use alloc::sync::Arc;
use core::cmp::Ordering;
use riscv::register::time;

const TICKS_PER_SEC: usize = 100;
const MSEC_PER_SEC: usize = 1000;

/// 读取 `mtime` 寄存器的值
pub fn get_time() -> usize {
    time::read()
}

/// 获取当前时间（单位：毫秒）
pub fn get_time_ms() -> usize {
    time::read() / (CLOCK_FREQ / MSEC_PER_SEC)
}

/// 设置下一次时钟中断的触发时间
pub fn set_next_trigger() {
    set_timer(get_time() + CLOCK_FREQ / TICKS_PER_SEC);
}

pub fn sleep_ms(ms: usize) {
    let start = get_time_ms();
    while get_time_ms() - start < ms {
        core::hint::spin_loop();
    }
}

/// 时间条件变量：包含一个过期时间和一个任务控制块。当过期时间到达时，相关的任务将被唤醒。
pub struct TimerCondVar {
    /// The time when the timer expires, in milliseconds
    pub expire_ms: usize,
    /// The task to be woken up when the timer expires
    pub task: Arc<ThreadControlBlock>,
}

impl PartialEq for TimerCondVar {
    fn eq(&self, other: &Self) -> bool {
        self.expire_ms == other.expire_ms && Arc::ptr_eq(&self.task, &other.task)
    }
}
impl Eq for TimerCondVar {}
impl PartialOrd for TimerCondVar {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TimerCondVar {
    fn cmp(&self, other: &Self) -> Ordering {
        other.expire_ms.cmp(&self.expire_ms)
    }
}

lazy_static::lazy_static! {
    /// TIMERS: global instance: set of timer condvars
    static ref TIMERS: SyncRefCell<BinaryHeap<TimerCondVar>> =
        unsafe { SyncRefCell::new(BinaryHeap::<TimerCondVar>::new()) };
}

pub fn add_timer(expire_ms: usize, task: Arc<ThreadControlBlock>) {
    let mut timers = TIMERS.exclusive_access();
    timers.push(TimerCondVar { expire_ms, task });
}

pub fn remove_timer(task: Arc<ThreadControlBlock>) {
    let mut timers = TIMERS.exclusive_access();
    timers.retain(|timer| !Arc::ptr_eq(&timer.task, &task));
}

pub fn check_timer() {
    poll_stdin();
    let current_ms = get_time_ms();
    let mut timers = TIMERS.exclusive_access();
    while let Some(timer) = timers.peek() {
        if timer.expire_ms <= current_ms {
            wakeup_task(timer.task.clone());
            timers.pop();
        } else {
            break;
        }
    }
}
