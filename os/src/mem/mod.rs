mod addr;
mod frame_allocator;
mod mem_set;
mod page_table;

pub use crate::mem::mem_set::MapPermission;
pub use addr::{PhysPageNum, VirtAddr};
use log::info;
pub use mem_set::KERNEL_SPACE;
pub use mem_set::MemorySet;
pub use page_table::{translated_byte_buffer, translated_refmut, translated_str};
mod heap_allocator;

pub fn init() {
    info!("初始化内存管理");
    heap_allocator::init_heap();
    frame_allocator::init_frame_allocator();
    KERNEL_SPACE.exclusive_access().activate();
}
