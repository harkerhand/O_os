#![no_std]
#![no_main]

#[macro_use]
extern crate user_lib;

use user_lib::{close, fork, pipe, read, wait, write};

static STR: &str = "Hello, world!";
static BLOCK_STR: [u8; 64] = [b'X'; 64];

#[unsafe(no_mangle)]
pub fn main() -> i32 {
    let mut pipe_fd = [0usize; 2];
    pipe(&mut pipe_fd);
    assert_eq!(pipe_fd[0], 3);
    assert_eq!(pipe_fd[1], 4);
    if fork() == 0 {
        // child process, read from parent
        close(pipe_fd[1]);
        let mut buffer = [0u8; 32];
        let len_read = read(pipe_fd[0], &mut buffer) as usize;
        close(pipe_fd[0]);
        assert_eq!(core::str::from_utf8(&buffer[..len_read]).unwrap(), STR);
        println!("Read OK, child process exited!");
        0
    } else {
        // parent process, write to child
        close(pipe_fd[0]);
        assert_eq!(write(pipe_fd[1], STR.as_bytes()), STR.len() as isize);
        close(pipe_fd[1]);
        let mut child_exit_code: i32 = 0;
        wait(&mut child_exit_code);
        assert_eq!(child_exit_code, 0);
        println!("pipetest passed!");

        // 回归测试：读端全部关闭后，写端应失败返回 -1，不应卡住。
        let mut pipe_fd2 = [0usize; 2];
        pipe(&mut pipe_fd2);
        println!("开始回归测试：关闭所有读端，再执行大块写入...");
        if fork() == 0 {
            close(pipe_fd2[0]);
            close(pipe_fd2[1]);
            return 0;
        } else {
            close(pipe_fd2[0]);
            let mut exit_code2: i32 = 0;
            wait(&mut exit_code2);
            println!("子进程已关闭读端，接下来写入 64 字节（管道容量 32）");
            assert_eq!(write(pipe_fd2[1], &BLOCK_STR), -1);
            close(pipe_fd2[1]);
            println!("pipe dead-reader test passed!");
        }
        0
    }
}
