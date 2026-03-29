use super::{BLOCK_SIZE, BlockDevice, get_block_cache};
use alloc::sync::Arc;
/// 一个位图块
type BitmapBlock = [u64; 64];
/// 一个块包含的位数
const BLOCK_BITS: usize = BLOCK_SIZE * 8;
/// 位图结构，包含起始块号和块数量
pub struct Bitmap {
    start_block_id: usize,
    blocks: usize,
}

/// 将一个 bit 的编号分解为块位置、64 位位置和位位置
fn decomposition(mut bit: usize) -> (usize, usize, usize) {
    let block_pos = bit / BLOCK_BITS;
    bit %= BLOCK_BITS;
    (block_pos, bit / 64, bit % 64)
}

impl Bitmap {
    /// 创建一个新的位图，参数是起始块号和块数量
    pub fn new(start_block_id: usize, blocks: usize) -> Self {
        Self {
            start_block_id,
            blocks,
        }
    }
    /// 分配一个块，返回分配的 bit 的编号
    pub fn alloc(&self, block_device: &Arc<dyn BlockDevice>) -> Option<usize> {
        for block_id in 0..self.blocks {
            let pos = get_block_cache(block_id + self.start_block_id, Arc::clone(block_device))
                .lock()
                .modify(0, |bitmap_block: &mut BitmapBlock| {
                    if let Some((bits64_pos, inner_pos)) = bitmap_block
                        .iter()
                        .enumerate()
                        .find(|(_, bits64)| **bits64 != u64::MAX)
                        .map(|(bits64_pos, bits64)| (bits64_pos, bits64.trailing_ones() as usize))
                    {
                        // modify cache
                        bitmap_block[bits64_pos] |= 1u64 << inner_pos;
                        Some(block_id * BLOCK_BITS + bits64_pos * 64 + inner_pos)
                    } else {
                        None
                    }
                });
            if pos.is_some() {
                return pos;
            }
        }
        None
    }
    /// 释放一个块，参数是要释放的 bit 的编号
    pub fn dealloc(&self, block_device: &Arc<dyn BlockDevice>, bit: usize) {
        let (block_pos, bits64_pos, inner_pos) = decomposition(bit);
        get_block_cache(block_pos + self.start_block_id, Arc::clone(block_device))
            .lock()
            .modify(0, |bitmap_block: &mut BitmapBlock| {
                assert!(bitmap_block[bits64_pos] & (1u64 << inner_pos) > 0);
                bitmap_block[bits64_pos] -= 1u64 << inner_pos;
            });
    }
    /// 返回位图的最大容量，即 blocks * BLOCK_BITS
    pub fn maximum(&self) -> usize {
        self.blocks * BLOCK_BITS
    }
}
