use alloc::vec::Vec;
use alloc::{string::String, vec};
use bitflags::bitflags;

use crate::config::PAGE_SIZE_BITS;
use crate::mem::KERNEL_SPACE;
use crate::mem::addr::PhysAddr;
use crate::mem::{
    VirtAddr,
    addr::{PhysPageNum, StepByOne, VirtPageNum},
    frame_allocator::{FrameTracker, frame_alloc},
};

bitflags! {
    pub struct PTEFlags: u8 {
        /// 页表项有效
        const V = 1 << 0;
        /// 页表项可读
        const R = 1 << 1;
        /// 页表项可写
        const W = 1 << 2;
        /// 页表项可执行
        const X = 1 << 3;
        /// 页表项用户态可访问
        const U = 1 << 4;
        /// 暂时IGNORE
        const G = 1 << 5;
        /// 访问过
        const A = 1 << 6;
        /// 脏页
        const D = 1 << 7;
    }
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct PageTableEntry {
    pub bits: usize,
}

impl PageTableEntry {
    pub fn new(ppn: PhysPageNum, flags: PTEFlags) -> Self {
        Self {
            bits: (ppn.0 << 10) | flags.bits() as usize,
        }
    }

    pub fn empty() -> Self {
        Self { bits: 0 }
    }

    pub fn ppn(&self) -> PhysPageNum {
        PhysPageNum(self.bits >> 10 & ((1usize << 44) - 1))
    }
    pub fn flags(&self) -> PTEFlags {
        PTEFlags::from_bits(self.bits as u8).unwrap()
    }
    pub fn is_valid(&self) -> bool {
        self.flags().contains(PTEFlags::V)
    }
    #[allow(unused)]
    pub fn readable(&self) -> bool {
        self.flags().contains(PTEFlags::R)
    }
    #[allow(unused)]
    pub fn writable(&self) -> bool {
        self.flags().contains(PTEFlags::W)
    }
    #[allow(unused)]
    pub fn executable(&self) -> bool {
        self.flags().contains(PTEFlags::X)
    }
}

pub struct PageTable {
    root_ppn: PhysPageNum,
    frames: Vec<FrameTracker>,
}

impl PageTable {
    pub fn new() -> Self {
        let frame = frame_alloc().unwrap();
        Self {
            root_ppn: frame.ppn,
            frames: vec![frame],
        }
    }

    pub fn map(&mut self, vpn: VirtPageNum, ppn: PhysPageNum, flags: PTEFlags) {
        let pte = self.find_pte_create(vpn).unwrap();
        assert!(
            !pte.is_valid(),
            "PageTable::map: vpn {vpn:?} is already mapped"
        );
        *pte = PageTableEntry::new(ppn, flags | PTEFlags::V);
    }
    pub fn unmap(&mut self, vpn: VirtPageNum) {
        let pte = self.find_pte_create(vpn).unwrap();
        assert!(
            pte.is_valid(),
            "PageTable::unmap: vpn {vpn:?} is not mapped"
        );
        *pte = PageTableEntry::empty();
    }

    fn find_pte_create(&mut self, vpn: VirtPageNum) -> Option<&mut PageTableEntry> {
        let idxs = vpn.indexes();
        let mut ppn = self.root_ppn;
        let mut result = None;

        for (i, &idx) in idxs.iter().enumerate() {
            let pte = &mut ppn.get_pte_array()[idx];
            if i == 2 {
                result = Some(pte);
                break;
            }
            if !pte.is_valid() {
                let frame = frame_alloc()?;
                *pte = PageTableEntry::new(frame.ppn, PTEFlags::V);
                self.frames.push(frame);
            }
            ppn = pte.ppn();
        }
        result
    }

    pub fn from_token(satp: usize) -> Self {
        Self {
            root_ppn: PhysPageNum(satp & ((1usize << 44) - 1)),
            frames: Vec::new(),
        }
    }

    fn find_pte(&self, vpn: VirtPageNum) -> Option<&PageTableEntry> {
        let idxs = vpn.indexes();
        let mut ppn = self.root_ppn;
        let mut result = None;
        for (i, &idx) in idxs.iter().enumerate() {
            let pte = &ppn.get_pte_array()[idx];
            if !pte.is_valid() {
                break;
            }
            if i == 2 {
                result = Some(pte);
                break;
            }
            ppn = pte.ppn();
        }
        result
    }

    pub fn translate(&self, vpn: VirtPageNum) -> Option<PageTableEntry> {
        self.find_pte(vpn).cloned()
    }

    pub fn translate_va(&self, va: VirtAddr) -> Option<PhysAddr> {
        self.find_pte(va.clone().floor()).map(|pte| {
            let aligned_pa: PhysAddr = pte.ppn().into();
            let offset = va.page_offset();
            let aligned_pa_usize: usize = aligned_pa.0;
            PhysAddr(aligned_pa_usize + offset)
        })
    }

    pub fn token(&self) -> usize {
        8usize << 60 | self.root_ppn.0
    }
}

/// 将用户空间的虚拟地址转换为内核空间的物理地址，并返回一个可变字节切片的向量
pub fn translated_byte_buffer(token: usize, ptr: *const u8, len: usize) -> Vec<&'static mut [u8]> {
    let page_table = PageTable::from_token(token);
    let mut start = ptr as usize;
    let end = start + len;
    let mut v = Vec::new();
    while start < end {
        let start_va = VirtAddr(start);
        let mut vpn = start_va.floor();
        let ppn = page_table.translate(vpn).unwrap().ppn();
        vpn.step();
        let mut end_va: VirtAddr = vpn.into();
        end_va = end_va.min(VirtAddr(end));
        if end_va.page_offset() == 0 {
            v.push(&mut ppn.get_bytes_array()[start_va.page_offset()..]);
        } else {
            v.push(&mut ppn.get_bytes_array()[start_va.page_offset()..end_va.page_offset()]);
        }
        start = end_va.0;
    }
    v
}

pub struct UserBuffer {
    pub buf: Vec<&'static mut [u8]>,
}

impl UserBuffer {
    pub fn new(buffers: Vec<&'static mut [u8]>) -> Self {
        Self { buf: buffers }
    }
    pub fn from_raw_parts(token: usize, ptr: *const u8, len: usize) -> Self {
        Self::new(translated_byte_buffer(token, ptr, len))
    }
    pub fn len(&self) -> usize {
        self.buf.iter().map(|b| b.len()).sum()
    }
}

pub fn translated_str(token: usize, ptr: *const u8) -> String {
    let page_table = PageTable::from_token(token);
    let mut string = String::new();
    let mut va = ptr as usize;
    loop {
        let ch: u8 = *(page_table.translate_va(VirtAddr(va)).unwrap().get_mut());
        if ch == 0 {
            break;
        } else {
            string.push(ch as char);
            va += 1;
        }
    }
    string
}

pub fn translated_refmut<T>(token: usize, ptr: *mut T) -> &'static mut T {
    let page_table = PageTable::from_token(token);
    let va = ptr as usize;
    page_table.translate_va(VirtAddr(va)).unwrap().get_mut()
}

pub fn translated_ref<T>(token: usize, ptr: *const T) -> &'static T {
    let page_table = PageTable::from_token(token);
    let va = ptr as usize;
    page_table.translate_va(VirtAddr(va)).unwrap().get_ref()
}
pub fn kernel_va_to_pa(va: usize) -> usize {
    let va = VirtAddr(va);
    let vpn = va.floor();
    let offset = va.page_offset();
    let pte = KERNEL_SPACE
        .exclusive_access()
        .translate(vpn)
        .expect("console_putstr: kernel va not mapped");
    (pte.ppn().0 << PAGE_SIZE_BITS) + offset
}
