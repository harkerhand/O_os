#![no_std]
#![no_main]

#[macro_use]
extern crate user_lib;

use user_lib::{close, fork, pipe, read, wait, write};

static STR: &str = "Hello, world!";
static BLOCK_STR: [u8; 64] = [b'X'; 64];
static FILL_STR: [u8; 32] = [b'P'; 32];

fn test_basic_pipe() {
    let mut pipe_fd = [0usize; 2];
    pipe(&mut pipe_fd);
    assert_eq!(pipe_fd[0], 3);
    assert_eq!(pipe_fd[1], 4);
    if fork() == 0 {
        close(pipe_fd[1]);
        let mut buffer = [0u8; 32];
        let len_read = read(pipe_fd[0], &mut buffer) as usize;
        close(pipe_fd[0]);
        assert_eq!(core::str::from_utf8(&buffer[..len_read]).unwrap(), STR);
        println!("Read OK, child process exited!");
        user_lib::exit(0);
    } else {
        close(pipe_fd[0]);
        assert_eq!(write(pipe_fd[1], STR.as_bytes()), STR.len() as isize);
        close(pipe_fd[1]);
        let mut child_exit_code: i32 = 0;
        wait(&mut child_exit_code);
        assert_eq!(child_exit_code, 0);
        println!("basic pipe test passed!");
    }
}

fn test_dead_reader() {
    let mut pipe_fd = [0usize; 2];
    pipe(&mut pipe_fd);
    println!("开始回归测试：关闭所有读端，再执行大块写入...");
    if fork() == 0 {
        close(pipe_fd[0]);
        close(pipe_fd[1]);
        user_lib::exit(0);
    } else {
        close(pipe_fd[0]);
        let mut exit_code: i32 = 0;
        wait(&mut exit_code);
        println!("子进程已关闭读端，接下来写入 64 字节（管道容量 32）");
        assert_eq!(write(pipe_fd[1], &BLOCK_STR), -1);
        close(pipe_fd[1]);
        println!("pipe dead-reader test passed!");
    }
}

fn test_blocked_reader_wakeup() {
    let mut pipe_fd = [0usize; 2];
    pipe(&mut pipe_fd);
    if fork() == 0 {
        close(pipe_fd[1]);
        let mut buffer = [0u8; 16];
        let len_read = read(pipe_fd[0], &mut buffer);
        close(pipe_fd[0]);
        assert_eq!(len_read, STR.len() as isize);
        assert_eq!(&buffer[..len_read as usize], STR.as_bytes());
        println!("blocked reader wakeup test passed in child!");
        user_lib::exit(0);
    } else {
        close(pipe_fd[0]);
        user_lib::sleep_ms(100);
        assert_eq!(write(pipe_fd[1], STR.as_bytes()), STR.len() as isize);
        close(pipe_fd[1]);
        let mut exit_code = 0;
        wait(&mut exit_code);
        assert_eq!(exit_code, 0);
        println!("blocked reader wakeup test passed!");
    }
}

fn test_blocked_writer_wakeup() {
    let mut pipe_fd = [0usize; 2];
    pipe(&mut pipe_fd);
    if fork() == 0 {
        close(pipe_fd[1]);
        user_lib::sleep_ms(100);
        let mut buffer = [0u8; 32];
        let len_read = read(pipe_fd[0], &mut buffer);
        assert_eq!(len_read, 32);
        assert_eq!(&buffer[..32], &FILL_STR);
        let len_read = read(pipe_fd[0], &mut buffer);
        assert_eq!(len_read, STR.len() as isize);
        assert_eq!(&buffer[..len_read as usize], STR.as_bytes());
        close(pipe_fd[0]);
        println!("blocked writer wakeup test passed in child!");
        user_lib::exit(0);
    } else {
        close(pipe_fd[0]);
        assert_eq!(write(pipe_fd[1], &FILL_STR), 32);
        assert_eq!(write(pipe_fd[1], STR.as_bytes()), STR.len() as isize);
        close(pipe_fd[1]);
        let mut exit_code = 0;
        wait(&mut exit_code);
        assert_eq!(exit_code, 0);
        println!("blocked writer wakeup test passed!");
    }
}

fn test_eof_wakeup() {
    let mut pipe_fd = [0usize; 2];
    pipe(&mut pipe_fd);
    if fork() == 0 {
        close(pipe_fd[1]);
        let mut buffer = [0u8; 8];
        let len_read = read(pipe_fd[0], &mut buffer);
        close(pipe_fd[0]);
        assert_eq!(len_read, 0);
        println!("EOF wakeup test passed in child!");
        user_lib::exit(0);
    } else {
        close(pipe_fd[0]);
        user_lib::sleep_ms(100);
        close(pipe_fd[1]);
        let mut exit_code = 0;
        wait(&mut exit_code);
        assert_eq!(exit_code, 0);
        println!("EOF wakeup test passed!");
    }
}

#[unsafe(no_mangle)]
pub fn main() -> i32 {
    test_basic_pipe();
    test_dead_reader();
    test_blocked_reader_wakeup();
    test_blocked_writer_wakeup();
    test_eof_wakeup();
    println!("pipetest passed!");
    0
}
