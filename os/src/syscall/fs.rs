//! 文件系统相关的系统调用

use crate::{
    mem::translated_byte_buffer,
    task::{current_user_token, suspend_current_and_run_next},
};

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
            assert_eq!(len, 1, "Only support reading one byte from stdin");
            let mut c: usize;
            loop {
                c = crate::sbi::console_getchar();
                if c != 0 {
                    break;
                }
                suspend_current_and_run_next();
                continue;
            }
            let ch = c as u8;
            let mut buffers = translated_byte_buffer(current_user_token(), buf, len);
            unsafe {
                buffers[0].as_mut_ptr().write_volatile(ch);
            }
            1
        }
        _ => {
            panic!("Unsupported fd in sys_read!");
        }
    }
}
