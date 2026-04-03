use crate::{
    config::PAGE_SIZE,
    task::{mmap_current, munmap_current},
};
use log::warn;

pub fn sys_mmap(start: usize, len: usize, prot: usize) -> isize {
    // 这里我们要求 start 必须是页对齐的，否则返回错误
    if start & (PAGE_SIZE - 1) != 0 {
        return -1;
    }
    if len == 0 {
        return 0;
    }
    // 这里我们要求 prot 的高 61 位必须为0，否则返回错误
    if prot >> 3 != 0 {
        return -1;
    }
    // 这里我们要求 prot 的低 3 位不能全为0，否则返回错误
    if prot & 0b111 == 0 {
        return -1;
    }
    let end = start.checked_add(len);
    if end.is_none() {
        return -1;
    }
    // 对end进行页对齐，如果end不是页对齐的，则向上取整到下一个页边界
    let end = (end.unwrap() + PAGE_SIZE - 1) & !(PAGE_SIZE - 1);

    let result = mmap_current(start, end, prot << 1);
    if result < 0 {
        warn!(
            "mmap 失败: start={:#x}, len={:#x}, prot={:#b}",
            start, len, prot
        );
    }
    result
}

pub fn sys_munmap(start: usize, len: usize) -> isize {
    // 这里我们要求 start 必须是页对齐的，否则返回错误
    if start & (PAGE_SIZE - 1) != 0 {
        return -1;
    }
    if len == 0 {
        return 0;
    }
    let end = start.checked_add(len);
    if end.is_none() {
        return -1;
    }
    // 对end进行页对齐，如果end不是页对齐的，则向上取整到下一个页边界
    let end = (end.unwrap() + PAGE_SIZE - 1) & !(PAGE_SIZE - 1);

    let result = munmap_current(start, end);
    if result < 0 {
        warn!("munmap 失败: start={:#x}, len={:#x}", start, len);
    }
    result
}
