//! 处理用户空间的中断、异常和系统调用
//! 当用户空间发生中断、异常或系统调用时，CPU 会自动切换到内核模式，
//! 并跳转到一个预定义的地址（由 stvec 寄存器指定）执行相应的处理程序。
//! 在这个处理程序中，我们需要根据 scause 寄存器的值来判断是什么类型的事件发生了，并进行相应的处理。
mod context;

use crate::{
    syscall::syscall,
    task::{exit_current_and_run_next, suspend_current_and_run_next},
    timer::set_next_trigger,
};
use core::arch::global_asm;
use log::{debug, error};
use riscv::{
    ExceptionNumber, InterruptNumber,
    interrupt::{Exception, Interrupt},
    register::{
        mtvec::TrapMode,
        scause::{self, Trap},
        sie, stval,
        stvec::{self, Stvec},
    },
};

pub use context::TrapContext;

global_asm!(include_str!("trap.S"));

/// 修改 stvec 寄存器，使其指向 __alltraps 函数
pub fn init() {
    unsafe extern "C" {
        safe fn __alltraps();
    }
    unsafe {
        stvec::write(Stvec::new(
            __alltraps as *const () as usize,
            TrapMode::Direct,
        ));
    }
}

#[unsafe(no_mangle)]
/// 处理用户空间的中断、异常和系统调用
pub fn trap_handler(cx: &mut TrapContext) -> &mut TrapContext {
    let scause = scause::read();
    let stval = stval::read();
    match scause.cause() {
        Trap::Exception(code) => {
            match Exception::from_number(code) {
                Ok(Exception::UserEnvCall) => {
                    cx.sepc += 4; // 跳过 ecall 指令，退出后继续执行下一条指令
                    cx.x[10] = syscall(cx.x[17], [cx.x[10], cx.x[11], cx.x[12]]) as usize;
                }
                Ok(Exception::StoreFault) | Ok(Exception::StorePageFault) => {
                    error!("[kernel] 应用页错误，内核杀死了它。");
                    exit_current_and_run_next();
                }
                Ok(Exception::IllegalInstruction) => {
                    error!("[kernel] 应用执行了非法指令，内核杀死了它。");
                    exit_current_and_run_next();
                }
                _ => {
                    panic!("未知异常 {:?}, stval = {:#x}!", scause.cause(), stval);
                }
            }
        }
        Trap::Interrupt(code) => {
            match Interrupt::from_number(code) {
                Ok(Interrupt::SupervisorTimer) => {
                    debug!("[kernel] 时间中断");
                    // 重新设置下一次 timer interrupt
                    set_next_trigger();
                    // 切换到下一个任务
                    suspend_current_and_run_next();
                }
                _ => {
                    panic!("未知中断 {:?}!", scause.cause());
                }
            }
        }
    }
    cx
}

/// timer interrupt enabled
pub fn enable_timer_interrupt() {
    unsafe {
        sie::set_stimer();
    }
}
