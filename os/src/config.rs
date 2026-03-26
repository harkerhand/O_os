//! 配置

pub const USER_STACK_SIZE: usize = 4096;
pub const KERNEL_STACK_SIZE: usize = 4096 * 2;

/// QEMU 的时钟频率为 12.5MHz
pub const CLOCK_FREQ: usize = 12500000;

/// 页大小为 4096 字节
pub const PAGE_SIZE: usize = 4096;
/// 页大小的位数，即 2^12 = 4096
pub const PAGE_SIZE_BITS: usize = 12;

/// 内存的结束地址
pub const MEMORY_END: usize = 0x8800_0000;

/// trampoline 的地址，位于内存的最后一页
pub const TRAMPOLINE: usize = usize::MAX - PAGE_SIZE + 1;

/// MMIO 设备的地址范围
pub const MMIO: &[(usize, usize)] = &[
    (0x0010_0000, 0x00_2000), // VIRT_TEST/RTC  in virt machine
];

/// trap context 的地址，位于 trampoline 的前一页
pub const TRAP_CONTEXT: usize = TRAMPOLINE - PAGE_SIZE;

/// 内核堆的大小
pub const KERNEL_HEAP_SIZE: usize = 0x300000;

/// 获取内核栈的位置，位于 trampoline 的前面，每个应用程序占用一段连续的内核栈空间
pub fn kernel_stack_position(app_id: usize) -> (usize, usize) {
    let top = TRAMPOLINE - app_id * (KERNEL_STACK_SIZE + PAGE_SIZE);
    let bottom = top - KERNEL_STACK_SIZE;
    (bottom, top)
}
