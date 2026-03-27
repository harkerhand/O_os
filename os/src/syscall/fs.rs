//! 文件系统相关的系统调用

use crate::{mem::translated_byte_buffer, task::current_user_token};

const FD_STDIN: usize = 0;
const FD_STDOUT: usize = 1;

/// 向文件描述符写入数据
pub fn sys_write(fd: usize, buf: *const u8, len: usize) -> isize {
    match fd {
        FD_STDOUT => {
            let buffers = translated_byte_buffer(current_user_token(), buf, len);
            for buffer in buffers {
                let str = core::str::from_utf8(buffer).unwrap_or("[Invalid UTF-8]");
                print!("{}", str);
            }
            len as isize
        }
        _ => {
            panic!("Unsupported fd in sys_write!");
        }
    }
}

pub fn sys_read(fd: usize, buf: *mut u8, len: usize) -> isize {
    match fd {
        FD_STDIN => {
            if len == 0 {
                return 0;
            }
            let buffers = translated_byte_buffer(current_user_token(), buf, len);
            let mut total_read = 0;
            for buffer in buffers {
                let read_len = crate::sbi::console_getchar(buffer.as_mut_ptr(), buffer.len());
                if read_len < 0 {
                    return if total_read > 0 { total_read } else { -1 };
                }
                if read_len == 0 {
                    break;
                }
                total_read += read_len;
                if read_len < buffer.len() as isize {
                    break;
                }
            }
            total_read
        }
        _ => {
            panic!("Unsupported fd in sys_read!");
        }
    }
}
