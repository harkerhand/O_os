//! SBI call wrappers

const SBI_CONSOLE_PUTCHAR: usize = 1;
const SBI_SHUTDOWN: usize = 8;

/// SBI 调用返回值
#[allow(dead_code)]
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

/// 使用 SBI 调用向底层输出一个字符
pub fn console_putchar(c: usize) {
    sbi_call(SBI_CONSOLE_PUTCHAR, 0, c, 0, 0);
}

/// 使用 SBI 调用关闭系统电源
pub fn shutdown() -> ! {
    sbi_call(SBI_SHUTDOWN, 0, 0, 0, 0);
    panic!("It should shutdown!");
}
