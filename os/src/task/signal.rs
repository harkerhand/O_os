//! 信号

bitflags::bitflags! {
    pub struct SignalFlags: u32 {
        /// Interrupt 中断信号
        const SIGINT = 1 << 2;
        /// Illegal Instruction 非法指令信号
        const SIGILL = 1 << 4;
        /// Abort 进程异常终止信号
        const SIGABRT = 1 << 6;
        /// Floating Point Exception 浮点异常信号
        const SIGFPE = 1 << 8;
        /// Segmentation Fault 内存访问错误信号
        const SIGSEGV = 1 << 11;
    }
}

impl SignalFlags {
    pub fn check_error(&self) -> Option<(i32, &'static str)> {
        if self.contains(SignalFlags::SIGINT) {
            Some((2, "Interrupt"))
        } else if self.contains(SignalFlags::SIGILL) {
            Some((4, "Illegal Instruction"))
        } else if self.contains(SignalFlags::SIGABRT) {
            Some((6, "Abort"))
        } else if self.contains(SignalFlags::SIGFPE) {
            Some((8, "Floating Point Exception"))
        } else if self.contains(SignalFlags::SIGSEGV) {
            Some((11, "Segmentation Fault"))
        } else {
            None
        }
    }
}
