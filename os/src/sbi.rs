//! SBI call wrappers

const SBI_CONSOLE_PUTSTR: usize = 0x4442434E; // "DBCN" in ASCII

#[allow(dead_code)]
/// SBI 调用返回值
#[derive(Copy, Clone, Debug)]
struct SbiRet {
    error: usize,
    value: usize,
}

/// 向底层 SBI 服务发起环境调用（Environment Call）。
///
/// 该函数遵循 RISC-V SBI 标准规范，将参数放入指定寄存器并触发 `ecall` 陷入 M-Mode。
///
/// # 参数
///
/// * `extension` - 扩展编号 (EID)，存放在 `a7` 寄存器。用于区分不同的 SBI 模块。
/// * `function`  - 函数编号 (FID)，存放在 `a6` 寄存器。用于区分模块内的具体功能。
/// * `arg0`      - 第一个参数，存放在 `a0` 寄存器。
/// * `arg1`      - 第二个参数，存放在 `a1` 寄存器。
/// * `arg2`      - 第三个参数，存放在 `a2` 寄存器。
///
/// # 返回值
///
/// 返回一个 [`SbiRet`] 结构体，包含：
/// * `error`: 存放在 `a0` 的错误码（0 表示成功）。
/// * `value`: 存放在 `a1` 的功能返回值。
///
/// # 安全性 (Safety)
///
/// 此函数是不安全的，因为它直接通过汇编指令与硬件/固件交互。调用者必须确保：
/// 1. 传入的 EID 和 FID 是目标平台支持的。
/// 2. 传入的参数（如内存地址）在当前特权级下是合法且有效的。
#[inline(always)]
fn sbi_call(extension: usize, function: usize, arg0: usize, arg1: usize, arg2: usize) -> SbiRet {
    let (error, value);
    unsafe {
        core::arch::asm!(
            "ecall",
            in("a7") extension,
            in("a6") function,
            inlateout("a0") arg0 => error,
            inlateout("a1") arg1 => value,
            in("a2") arg2,
        );
    }
    SbiRet { error, value }
}

#[allow(dead_code)]
/// 使用 SBI 调用检查底层是否支持某个扩展
pub fn probe_extension(extension: usize) -> bool {
    let ret = sbi_call(0x10, 3, extension, 0, 0);
    ret.error == 0 && ret.value != 0
}

#[allow(dead_code)]
/// 使用 SBI 调用向底层输出一个字符
pub fn console_putchar(c: u8) {
    sbi_call(SBI_CONSOLE_PUTSTR, 2, c as usize, 0, 0);
}

/// 使用 SBI 调用向底层输出一个字符串
pub fn console_putstr(s: &str) {
    let len = s.len();
    let ptr = s.as_ptr() as usize;
    sbi_call(SBI_CONSOLE_PUTSTR, 0, len, ptr, 0);
}

const SBI_EXT_SRST: usize = 0x53525354;
const SBI_SRST_RESET: usize = 0;

/// 使用 SRST 扩展关闭系统或重启
pub fn system_reset(reset_type: usize, reason: usize) -> ! {
    sbi_call(SBI_EXT_SRST, SBI_SRST_RESET, reset_type, reason, 0);
    panic!("It should shutdown")
}

/// 正常关机
/// Type 0: Shutdown, Reason 0: No Reason
pub fn shutdown() -> ! {
    // Type 0: Shutdown, Reason 0: No Reason
    system_reset(0, 0);
}

#[allow(dead_code)]
/// 故障关机
/// Type 0: Shutdown, Reason 1: System Failure
pub fn panic_shutdown() -> ! {
    system_reset(0, 1);
}

const SBI_EXT_TIME: usize = 0x54494D45; // "TIME" in ASCII
const SBI_TIME_SET_TIMER: usize = 0;

/// SBI 调用设置下一个定时器触发时间
pub fn set_timer(stime_value: usize) {
    // 根据 SBI v0.2+ 规范：
    // a7: EID (0x54494D45)
    // a6: FID (0)
    // a0: stime_value (64位计数值)
    sbi_call(SBI_EXT_TIME, SBI_TIME_SET_TIMER, stime_value, 0, 0);
}
