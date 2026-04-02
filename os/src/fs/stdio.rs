//! 标准输入输出

use alloc::{collections::vec_deque::VecDeque, sync::Arc};

use crate::{
    fs::File,
    sync::SyncRefCell,
    task::{current_task, mark_current_blocked, schedule, wakeup_task},
};

pub struct Stdin;
pub struct Stdout;

pub use Stdout as Stderr;

struct StdinState {
    buffer: VecDeque<u8>,
    wait_queue: VecDeque<Arc<crate::task::ThreadControlBlock>>,
}

lazy_static::lazy_static! {
    static ref STDIN_STATE: SyncRefCell<StdinState> = unsafe {
        SyncRefCell::new(StdinState {
            buffer: VecDeque::new(),
            wait_queue: VecDeque::new(),
        })
    };
}

pub fn poll_stdin() {
    let mut local_buf = [0u8; 16];
    let read_len = crate::sbi::console_getchar(local_buf.as_mut_ptr(), local_buf.len());
    if read_len <= 0 {
        return;
    }
    let mut state = STDIN_STATE.exclusive_access();
    for byte in &local_buf[..read_len as usize] {
        state.buffer.push_back(*byte);
    }
    while !state.buffer.is_empty() {
        let Some(task) = state.wait_queue.pop_front() else {
            break;
        };
        wakeup_task(task);
    }
}

impl File for Stdin {
    fn readable(&self) -> bool {
        true
    }

    fn writeable(&self) -> bool {
        false
    }

    fn read(&self, buf: crate::mem::UserBuffer) -> isize {
        let mut total_read = 0isize;
        for buffer in buf.buf {
            let mut filled = 0usize;
            while filled < buffer.len() {
                let mut state = STDIN_STATE.exclusive_access();
                if let Some(byte) = state.buffer.pop_front() {
                    buffer[filled] = byte;
                    filled += 1;
                    total_read += 1;
                    continue;
                }
                if total_read > 0 {
                    return total_read;
                }
                let task = current_task();
                let task_cx_ptr = mark_current_blocked();
                state.wait_queue.push_back(task);
                drop(state);
                schedule(task_cx_ptr);
            }
        }
        total_read
    }

    fn write(&self, _buf: crate::mem::UserBuffer) -> isize {
        unimplemented!("不支持向标准输入写入数据")
    }
}

impl File for Stdout {
    fn readable(&self) -> bool {
        false
    }

    fn writeable(&self) -> bool {
        true
    }

    fn read(&self, _buf: crate::mem::UserBuffer) -> isize {
        unimplemented!("不支持从标准输出读取数据")
    }

    fn write(&self, buf: crate::mem::UserBuffer) -> isize {
        for buffer in &buf.buf {
            print!("{}", core::str::from_utf8(buffer).unwrap());
        }
        buf.len() as isize
    }
}
