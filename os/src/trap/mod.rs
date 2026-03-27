//! 处理用户空间的中断、异常和系统调用
//! 当用户空间发生中断、异常或系统调用时，CPU 会自动切换到内核模式，
//! 并跳转到一个预定义的地址（由 stvec 寄存器指定）执行相应的处理程序。
//! 在这个处理程序中，我们需要根据 scause 寄存器的值来判断是什么类型的事件发生了，并进行相应的处理。
mod context;

use crate::{
    config::{TRAMPOLINE, TRAP_CONTEXT},
    syscall::syscall,
    task::{
        current_trap_cx, current_user_token, exit_current_and_run_next,
        suspend_current_and_run_next,
    },
    timer::set_next_trigger,
};
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
core::arch::global_asm!(include_str!("trap.S"));

/// 修改 stvec 寄存器，使其指向 __alltraps 函数
pub fn init() {
    set_kernel_trap_entry();
}

fn set_kernel_trap_entry() {
    unsafe {
        stvec::write(Stvec::new(
            trap_from_kernel as *const () as usize,
            TrapMode::Direct,
        ));
    }
}

fn set_user_trap_entry() {
    unsafe {
        stvec::write(Stvec::new(TRAMPOLINE, TrapMode::Direct));
    }
}
#[unsafe(no_mangle)]
/// Unimplement: traps/interrupts/exceptions from kernel mode
/// Todo: Chapter 9: I/O device
pub fn trap_from_kernel() -> ! {
    panic!("a trap from kernel!");
}

#[unsafe(no_mangle)]
/// 处理用户空间的中断、异常和系统调用
pub fn trap_handler() -> ! {
    set_kernel_trap_entry();
    let cx = current_trap_cx();
    let scause = scause::read();
    let stval = stval::read();
    match scause.cause() {
        Trap::Exception(code) => {
            match Exception::from_number(code) {
                Ok(Exception::UserEnvCall) => {
                    cx.sepc += 4; // 跳过 ecall 指令，退出后继续执行下一条指令
                    cx.x[10] = syscall(cx.x[17], [cx.x[10], cx.x[11], cx.x[12]]) as usize;
                }
                Ok(Exception::StoreFault)
                | Ok(Exception::StorePageFault)
                | Ok(Exception::LoadFault)
                | Ok(Exception::LoadPageFault) => {
                    error!(
                        "应用页错误，内核杀死了它。错误地址 = {:#x}，错误指令 = {:#x}",
                        stval, cx.sepc
                    );
                    exit_current_and_run_next();
                }
                Ok(Exception::IllegalInstruction) => {
                    error!("应用执行了非法指令，内核杀死了它。");
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
                    debug!("时间中断");
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
    trap_return();
}

/// timer interrupt enabled
pub fn enable_timer_interrupt() {
    unsafe {
        sie::set_stimer();
    }
}

#[unsafe(no_mangle)]
/// set the new addr of __restore asm function in TRAMPOLINE page,
/// set the reg a0 = trap_cx_ptr, reg a1 = phy addr of usr page table,
/// finally, jump to new addr of __restore asm function
pub fn trap_return() -> ! {
    set_user_trap_entry();
    let trap_cx_ptr = TRAP_CONTEXT;
    let user_satp = current_user_token();
    unsafe extern "C" {
        unsafe fn __alltraps();
        unsafe fn __restore();
    }
    let restore_va =
        __restore as *const () as usize - __alltraps as *const () as usize + TRAMPOLINE;
    unsafe {
        core::arch::asm!(
            "fence.i",
            "jr {restore_va}",             // jump to new addr of __restore asm function
            restore_va = in(reg) restore_va,
            in("a0") trap_cx_ptr,      // a0 = virt addr of Trap Context
            in("a1") user_satp,        // a1 = phy addr of usr page table
            options(noreturn)
        );
    }
}
