use riscv::register::sstatus::{self, SPP, Sstatus};
/// Trap 上下文
#[repr(C)]
pub struct TrapContext {
    /// x0 - x31 寄存器
    pub x: [usize; 32],
    /// CSR sstatus  
    pub sstatus: Sstatus,
    /// CSR sepc
    pub sepc: usize,
    /// 内核页表地址
    pub kernel_satp: usize,
    /// 内核栈指针
    pub kernel_sp: usize,
    /// 内核 Trap 处理函数地址
    pub trap_handler: usize,
}

impl TrapContext {
    /// 设置用户栈指针到 x2 寄存器
    pub fn set_sp(&mut self, sp: usize) {
        self.x[2] = sp;
    }
    /// 创建应用程序的 TrapContext
    pub fn app_init_context(
        entry: usize,
        sp: usize,
        kernel_satp: usize,
        kernel_sp: usize,
        trap_handler: usize,
    ) -> Self {
        let mut sstatus = sstatus::read(); // 读取当前的 sstatus 寄存器值
        sstatus.set_spp(SPP::User); // 设置 sstatus 的 SPP 位为 User 模式
        let mut cx = Self {
            x: [0; 32],
            sstatus,
            sepc: entry, // 设置 sepc 寄存器为应用程序的入口地址
            kernel_satp,
            kernel_sp,
            trap_handler,
        };
        cx.set_sp(sp); // 设置用户栈指针
        cx // 返回初始化好的 TrapContext
    }
}
