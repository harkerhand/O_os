use core::any::Any;

/// 读写块设备的接口
pub trait BlockDevice: Send + Sync + Any {
    /// 从块设备中读取一个块的数据到 buf 中
    fn read_block(&self, block_id: usize, buf: &mut [u8]);
    /// 将 buf 中的数据写入块设备的一个块中
    fn write_block(&self, block_id: usize, buf: &[u8]);
}
