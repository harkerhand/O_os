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
    /// 页表项标志非法
    InvalidPteFlags,
    /// 映射区域权限非法
    InvalidMapPermission,
    /// 虚拟地址范围非法
    InvalidMapRange,
    /// 映射区域发生重叠
    MapAreaOverlap,
    /// 要取消映射的区域不存在
    MunmapAreaNotFound,
    /// 页表 walk 失败
    PageTableWalkFailed,
    /// 虚拟页已经被映射
    PageAlreadyMapped,
    /// 虚拟页尚未映射
    PageNotMapped,
    /// 虚拟地址未映射
    VirtAddrNotMapped,
}
