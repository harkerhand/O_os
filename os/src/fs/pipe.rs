//! 管道

use alloc::{
    collections::vec_deque::VecDeque,
    sync::{Arc, Weak},
};
use spin::Mutex;

use crate::{
    fs::File,
    task::{ThreadControlBlock, current_task, mark_current_blocked, schedule, wakeup_task},
};

pub struct Pipe {
    readable: bool,
    writeable: bool,
    buffer: Arc<Mutex<PipeRingBuffer>>,
}

impl File for Pipe {
    fn readable(&self) -> bool {
        self.readable
    }

    fn writeable(&self) -> bool {
        self.writeable
    }

    fn read(&self, buf: crate::mem::UserBuffer) -> isize {
        assert!(self.readable());
        let want_to_read = buf.len() as isize;
        let mut buf_iter = buf.into_iter();
        let mut already_read = 0;
        loop {
            let mut ring_buffer = self.buffer.lock();
            let loop_read = ring_buffer.available_read();
            if loop_read == 0 {
                if ring_buffer.all_write_ends_closed() {
                    return already_read;
                }
                let task = current_task();
                let task_cx_ptr = mark_current_blocked();
                ring_buffer.read_wait_queue.push_back(task);
                drop(ring_buffer);
                schedule(task_cx_ptr);
                continue;
            }
            for _ in 0..loop_read {
                if let Some(byte_ref) = buf_iter.next() {
                    unsafe {
                        *byte_ref = ring_buffer.read_byte();
                    }
                    already_read += 1;
                    if already_read == want_to_read {
                        ring_buffer.wake_one_writer();
                        return want_to_read;
                    }
                } else {
                    ring_buffer.wake_one_writer();
                    return already_read;
                }
            }
            ring_buffer.wake_one_writer();
        }
    }

    fn write(&self, buf: crate::mem::UserBuffer) -> isize {
        assert!(self.writeable());
        let want_to_write = buf.len() as isize;
        let mut buf_iter = buf.into_iter();
        let mut already_write = 0;
        loop {
            let mut ring_buffer = self.buffer.lock();
            if ring_buffer.all_read_ends_closed() {
                return if already_write > 0 { already_write } else { -1 };
            }
            let loop_write = ring_buffer.available_write();
            if loop_write == 0 {
                let task = current_task();
                let task_cx_ptr = mark_current_blocked();
                ring_buffer.write_wait_queue.push_back(task);
                drop(ring_buffer);
                schedule(task_cx_ptr);
                continue;
            }
            for _ in 0..loop_write {
                if let Some(byte_ref) = buf_iter.next() {
                    ring_buffer.write_byte(unsafe { *byte_ref });
                    already_write += 1;
                    if already_write == want_to_write {
                        ring_buffer.wake_one_reader();
                        return want_to_write;
                    }
                } else {
                    ring_buffer.wake_one_reader();
                    return already_write;
                }
            }
            ring_buffer.wake_one_reader();
        }
    }
}

const RING_BUFFER_SIZE: usize = 32;

pub struct PipeRingBuffer {
    arr: [u8; RING_BUFFER_SIZE],
    head: usize,
    tail: usize,
    count: usize,
    read_wait_queue: VecDeque<Arc<ThreadControlBlock>>,
    write_wait_queue: VecDeque<Arc<ThreadControlBlock>>,
    read_end: Option<Weak<Pipe>>,
    write_end: Option<Weak<Pipe>>,
}

impl PipeRingBuffer {
    pub fn new() -> Self {
        Self {
            arr: [0; RING_BUFFER_SIZE],
            head: 0,
            tail: 0,
            count: 0,
            read_wait_queue: VecDeque::new(),
            write_wait_queue: VecDeque::new(),
            read_end: None,
            write_end: None,
        }
    }

    pub fn set_read_end(&mut self, read_end: &Arc<Pipe>) {
        self.read_end = Some(Arc::downgrade(read_end));
    }

    pub fn set_write_end(&mut self, write_end: &Arc<Pipe>) {
        self.write_end = Some(Arc::downgrade(write_end));
    }

    pub fn read_byte(&mut self) -> u8 {
        assert!(self.count > 0);
        let byte = self.arr[self.head];
        self.head = (self.head + 1) % RING_BUFFER_SIZE;
        self.count -= 1;
        byte
    }

    pub fn available_read(&self) -> usize {
        self.count
    }

    pub fn write_byte(&mut self, byte: u8) {
        assert!(self.count < RING_BUFFER_SIZE);
        self.arr[self.tail] = byte;
        self.tail = (self.tail + 1) % RING_BUFFER_SIZE;
        self.count += 1;
    }

    pub fn available_write(&self) -> usize {
        RING_BUFFER_SIZE - self.count
    }

    fn wake_all(wait_queue: &mut VecDeque<Arc<ThreadControlBlock>>) {
        while let Some(task) = wait_queue.pop_front() {
            wakeup_task(task);
        }
    }

    pub fn wake_one_reader(&mut self) {
        Self::wake_all(&mut self.read_wait_queue);
    }

    pub fn wake_one_writer(&mut self) {
        Self::wake_all(&mut self.write_wait_queue);
    }

    pub fn wake_all_readers(&mut self) {
        Self::wake_all(&mut self.read_wait_queue);
    }

    pub fn wake_all_writers(&mut self) {
        Self::wake_all(&mut self.write_wait_queue);
    }

    pub fn all_write_ends_closed(&self) -> bool {
        if let Some(write_end) = &self.write_end {
            write_end.upgrade().is_none()
        } else {
            true
        }
    }

    pub fn all_read_ends_closed(&self) -> bool {
        if let Some(read_end) = &self.read_end {
            read_end.upgrade().is_none()
        } else {
            true
        }
    }
}

impl Pipe {
    pub fn read_end_with_buffer(buffer: Arc<Mutex<PipeRingBuffer>>) -> Self {
        Self {
            readable: true,
            writeable: false,
            buffer,
        }
    }

    pub fn write_end_with_buffer(buffer: Arc<Mutex<PipeRingBuffer>>) -> Self {
        Self {
            readable: false,
            writeable: true,
            buffer,
        }
    }
}

impl Drop for Pipe {
    fn drop(&mut self) {
        let mut ring_buffer = self.buffer.lock();
        if self.readable {
            ring_buffer.wake_all_writers();
        }
        if self.writeable {
            ring_buffer.wake_all_readers();
        }
    }
}

/// 创建一个管道，返回读写两端的 Pipe 对象
pub fn make_pipe() -> (Arc<Pipe>, Arc<Pipe>) {
    let buffer = Arc::new(Mutex::new(PipeRingBuffer::new()));
    let read_end = Arc::new(Pipe::read_end_with_buffer(buffer.clone()));
    let write_end = Arc::new(Pipe::write_end_with_buffer(buffer.clone()));
    let mut ring = buffer.lock();
    ring.set_read_end(&read_end);
    ring.set_write_end(&write_end);
    drop(ring);
    (read_end, write_end)
}
