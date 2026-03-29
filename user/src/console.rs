use crate::sync::SpinLock;

use super::write;
use core::fmt::{self, Write};

struct Stdout;

const STDOUT: usize = 1;

impl Write for Stdout {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        write(STDOUT, s.as_bytes());
        Ok(())
    }
}

pub fn print(args: fmt::Arguments) {
    Stdout.write_fmt(args).unwrap();
}

#[macro_export]
macro_rules! print {
    ($fmt: literal $(, $($arg: tt)+)?) => {
        $crate::console::print(format_args!($fmt $(, $($arg)+)?));
    }
}

#[macro_export]
macro_rules! println {
    ($fmt: literal $(, $($arg: tt)+)?) => {
        $crate::console::print(format_args!(concat!($fmt, "\n") $(, $($arg)+)?));
    }
}

#[macro_export]
macro_rules! blue {
    ($fmt: literal $(, $($arg: tt)+)?) => {
        $crate::console::print(format_args!("\x1b[34m{} \x1b[0m", format_args!($fmt $(, $($arg)+)?)));
    }
}
#[macro_export]
macro_rules! blueln {
    ($fmt: literal $(, $($arg: tt)+)?) => {
        $crate::console::print(format_args!("\x1b[34m{} \x1b[0m\n", format_args!($fmt $(, $($arg)+)?)));
    }
}

pub struct Stdin {
    buffer: [u8; 128],
    pos: usize,
    len: usize,
}
impl Stdin {
    pub fn getchar(&mut self) -> u8 {
        loop {
            if self.pos < self.len {
                let c = self.buffer[self.pos];
                self.pos += 1;
                return c;
            }
            let read_len = crate::read(0, &mut self.buffer);
            if read_len < 0 {
                return 0;
            }
            if read_len == 0 {
                crate::yield_();
                continue;
            }
            self.pos = 0;
            self.len = read_len as usize;
        }
    }
}

static STDIN: SpinLock<Stdin> = SpinLock::new(Stdin {
    buffer: [0; 128],
    pos: 0,
    len: 0,
});

pub fn getchar() -> u8 {
    STDIN.lock().getchar()
}
