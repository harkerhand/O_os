//! 错误类型定义

pub type KernelResult<T> = core::result::Result<T, KernelError>;

#[derive(Debug)]
pub enum KernelError {
    /// 释放过多内存
    ReleaseTooMuch,
    /// 释放的虚拟地址未找到
    ShrinkVirtAddrNotFound,
    /// 申请的虚拟地址未找到
    AppendVirtAddrNotFound,
    /// 申请帧失败
    FrameAllocFailed,
}
