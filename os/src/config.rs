//! 配置

/// QEMU 的时钟频率为 12.5MHz
pub const CLOCK_FREQ: usize = 12500000;

/// 页大小为 4096 字节
pub const PAGE_SIZE: usize = 4096;
/// 页大小的位数，即 2^12 = 4096
pub const PAGE_SIZE_BITS: usize = 12;

/// 内存的结束地址，可用地址空间为 Ox8000_0000 ~ 0x8800_0000，大小为 128MB
pub const MEMORY_END: usize = 0x8800_0000;

/// trampoline 的地址，位于内存的最后一页
pub const TRAMPOLINE: usize = usize::MAX - PAGE_SIZE + 1;

/// trap context 的地址，位于 trampoline 的前一页
pub const TRAP_CONTEXT: usize = TRAMPOLINE - PAGE_SIZE;

/// 用户栈的大小
pub const USER_STACK_SIZE: usize = 4096;
/// 用户栈顶，即TrapContext的位置，向下生长
pub const USER_STACK_TOP: usize = TRAP_CONTEXT;
/// 用户栈底，位于trap context前
pub const USER_STACK_BOTTOM: usize = TRAP_CONTEXT - USER_STACK_SIZE;

/// 内核堆的大小，由堆管理器控制，大小为 16MB
pub const KERNEL_HEAP_SIZE: usize = 0x1000000;

/// 内核栈的大小，每个应用程序占用 8KB 的内核栈空间
pub const KERNEL_STACK_SIZE: usize = 4096 * 2;
