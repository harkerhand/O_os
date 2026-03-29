//! 标准输入输出

use crate::fs::File;
use crate::task::suspend_current_and_run_next;

pub struct Stdin;
pub struct Stdout;

pub use Stdout as Stderr;

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
            loop {
                let read_len = crate::sbi::console_getchar(buffer.as_mut_ptr(), buffer.len());
                if read_len < 0 {
                    return if total_read > 0 { total_read } else { -1 };
                }
                if read_len == 0 {
                    if total_read > 0 {
                        return total_read;
                    }
                    suspend_current_and_run_next();
                    continue;
                }
                total_read += read_len;
                if read_len < buffer.len() as isize {
                    return total_read;
                }
                break;
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
