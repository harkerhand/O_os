//! 文件系统相关的系统调用

use crate::{mem::translated_byte_buffer, task::current_user_token};

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
