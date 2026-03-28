use super::BlockDevice;
use crate::config::PAGE_SIZE_BITS;
use crate::mem::{FrameTracker, frame_alloc, kernel_va_to_pa};
use crate::sync::SyncRefCell;
use alloc::vec::Vec;
use core::ptr::NonNull;
use lazy_static::*;
use virtio_drivers::device::blk::VirtIOBlk;
use virtio_drivers::transport::mmio::{MmioTransport, VirtIOHeader};
use virtio_drivers::{BufferDirection, Hal, PhysAddr};

#[allow(unused)]
const VIRTIO0: usize = 0x10001000;

pub struct VirtIOBlock(SyncRefCell<VirtIOBlk<VirtioHal, MmioTransport<'static>>>);

lazy_static! {
    static ref QUEUE_FRAMES: SyncRefCell<Vec<FrameTracker>> =
        unsafe { SyncRefCell::new(Vec::new()) };
}

impl BlockDevice for VirtIOBlock {
    fn read_block(&self, block_id: usize, buf: &mut [u8]) {
        self.0
            .exclusive_access()
            .read_blocks(block_id, buf)
            .expect("Error when reading VirtIOBlk");
    }
    fn write_block(&self, block_id: usize, buf: &[u8]) {
        self.0
            .exclusive_access()
            .write_blocks(block_id, buf)
            .expect("Error when writing VirtIOBlk");
    }
}

impl VirtIOBlock {
    #[allow(unused)]
    pub fn new() -> Self {
        unsafe {
            let header = NonNull::new(VIRTIO0 as *mut VirtIOHeader).unwrap();
            let transport = MmioTransport::new(header, 0x1000).unwrap();
            Self(SyncRefCell::new(
                VirtIOBlk::<VirtioHal, MmioTransport>::new(transport).unwrap(),
            ))
        }
    }
}

pub struct VirtioHal;

unsafe impl Hal for VirtioHal {
    fn dma_alloc(pages: usize, _direction: BufferDirection) -> (PhysAddr, NonNull<u8>) {
        let mut ppn_base = 0usize;
        for i in 0..pages {
            let frame = frame_alloc().unwrap();
            if i == 0 {
                ppn_base = frame.ppn.0;
            }
            assert_eq!(frame.ppn.0, ppn_base + i);
            QUEUE_FRAMES.exclusive_access().push(frame);
        }
        let paddr = (ppn_base << PAGE_SIZE_BITS) as PhysAddr;
        let vaddr = NonNull::new(paddr as usize as *mut u8).unwrap();
        (paddr, vaddr)
    }

    unsafe fn dma_dealloc(paddr: PhysAddr, _vaddr: NonNull<u8>, pages: usize) -> i32 {
        let ppn_base = (paddr as usize) >> PAGE_SIZE_BITS;
        let mut frames = QUEUE_FRAMES.exclusive_access();
        for i in 0..pages {
            let target = ppn_base + i;
            if let Some(pos) = frames.iter().position(|f| f.ppn.0 == target) {
                frames.swap_remove(pos);
            } else {
                return -1;
            }
        }
        0
    }

    unsafe fn mmio_phys_to_virt(paddr: PhysAddr, _size: usize) -> NonNull<u8> {
        NonNull::new(paddr as usize as *mut u8).unwrap()
    }

    unsafe fn share(buffer: NonNull<[u8]>, _direction: BufferDirection) -> PhysAddr {
        let vaddr = buffer.as_ptr() as *const u8 as usize;
        let len = unsafe { buffer.as_ref().len() };

        let first = kernel_va_to_pa(vaddr);
        if len <= 1 {
            return first as PhysAddr;
        }

        let last_vaddr = vaddr + len - 1;
        let first_page = vaddr >> PAGE_SIZE_BITS;
        let last_page = last_vaddr >> PAGE_SIZE_BITS;
        if first_page != last_page {
            let mut prev_page_pa = first & !((1usize << PAGE_SIZE_BITS) - 1);
            for page in (first_page + 1)..=last_page {
                let page_va = page << PAGE_SIZE_BITS;
                let curr_page_pa = kernel_va_to_pa(page_va);
                assert_eq!(
                    curr_page_pa,
                    prev_page_pa + (1usize << PAGE_SIZE_BITS),
                    "virtio share: 缓冲区跨页但物理不连续"
                );
                prev_page_pa = curr_page_pa;
            }
        }
        first as PhysAddr
    }

    unsafe fn unshare(_paddr: PhysAddr, _buffer: NonNull<[u8]>, _direction: BufferDirection) {}
}
