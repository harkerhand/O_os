use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::arch::asm;
use lazy_static::*;
use log::{debug, info};
use riscv::register::satp::{self, Satp};

use crate::config::{MEMORY_END, PAGE_SIZE, TRAMPOLINE};
use crate::error::{KernelError, KernelResult};
use crate::mem::addr::{PhysAddr, PhysPageNum, StepByOne, VPNRange, VirtAddr, VirtPageNum};
use crate::mem::frame_allocator::{FrameTracker, frame_alloc};
use crate::mem::page_table::{PTEFlags, PageTable, PageTableEntry};
use crate::sync::SyncRefCell;

unsafe extern "C" {
    safe fn stext();
    safe fn etext();
    safe fn srodata();
    safe fn erodata();
    safe fn sdata();
    safe fn edata();
    safe fn sbss_with_stack();
    safe fn ebss();
    safe fn ekernel();
    safe fn strampoline();
}

lazy_static! {
    /// 内核空间的内存集，包含内核所有的虚拟地址空间
    pub static ref KERNEL_SPACE: Arc<SyncRefCell<MemorySet>> =
        Arc::new(unsafe { SyncRefCell::new(MemorySet::new_kernel()) });
}

/// 地址空间集合，包含一个页表和多个映射区域
pub struct MemorySet {
    page_table: PageTable,
    areas: Vec<MapArea>,
}

impl MemorySet {
    pub fn new_bare() -> Self {
        Self {
            page_table: PageTable::new(),
            areas: Vec::new(),
        }
    }
    pub fn from_another(other: &MemorySet) -> Self {
        let mut memory_set = Self::new_bare();
        memory_set.map_trampoline();
        for area in &other.areas {
            let new_area = MapArea::from_another(area);
            memory_set.push(new_area, None);
            for vpn in area.vpn_range {
                let src_ppn = other.translate(vpn).unwrap().ppn();
                let dst_ppn = memory_set.translate(vpn).unwrap().ppn();
                dst_ppn
                    .get_bytes_array()
                    .copy_from_slice(src_ppn.get_bytes_array());
            }
        }
        memory_set
    }

    pub fn recycle_data_pages(&mut self) {
        self.areas.clear();
    }

    pub fn token(&self) -> usize {
        self.page_table.token()
    }
    /// 假设不冲突
    pub fn insert_framed_area(
        &mut self,
        start_va: VirtAddr,
        end_va: VirtAddr,
        permission: MapPermission,
    ) {
        self.push(
            MapArea::new(start_va, end_va, MapType::Framed, permission),
            None,
        );
    }
    fn push(&mut self, mut map_area: MapArea, data: Option<&[u8]>) {
        map_area.map(&mut self.page_table);
        if let Some(data) = data {
            map_area.copy_data(&self.page_table, data);
        }
        self.areas.push(map_area);
    }
    /// 提醒 trampoline 不被 areas 收集
    fn map_trampoline(&mut self) {
        self.page_table.map(
            VirtAddr(TRAMPOLINE).into(),
            PhysAddr(strampoline as *const () as usize).into(),
            PTEFlags::R | PTEFlags::X,
        );
    }
    /// 创建内核空间的内存集，包含 trampoline 和 elf 中的各个段
    pub fn new_kernel() -> Self {
        let mut memory_set = Self::new_bare();
        // 映射 trampoline
        memory_set.map_trampoline();
        // 映射 elf 中的各个段，权限根据段类型设置
        debug!(
            ".text [{:#x}, {:#x})",
            stext as *const () as usize, etext as *const () as usize
        );
        debug!(
            ".rodata [{:#x}, {:#x})",
            srodata as *const () as usize, erodata as *const () as usize
        );
        debug!(
            ".data [{:#x}, {:#x})",
            sdata as *const () as usize, edata as *const () as usize
        );
        debug!(
            ".bss [{:#x}, {:#x})",
            sbss_with_stack as *const () as usize, ebss as *const () as usize
        );
        debug!("mapping .text section");
        memory_set.push(
            MapArea::new(
                VirtAddr(stext as *const () as usize),
                VirtAddr(etext as *const () as usize),
                MapType::Identical,
                MapPermission::R | MapPermission::X,
            ),
            None,
        );
        debug!("mapping .rodata section");
        memory_set.push(
            MapArea::new(
                VirtAddr(srodata as *const () as usize),
                VirtAddr(erodata as *const () as usize),
                MapType::Identical,
                MapPermission::R,
            ),
            None,
        );
        debug!("mapping .data section");
        memory_set.push(
            MapArea::new(
                VirtAddr(sdata as *const () as usize),
                VirtAddr(edata as *const () as usize),
                MapType::Identical,
                MapPermission::R | MapPermission::W,
            ),
            None,
        );
        debug!("mapping .bss section");
        memory_set.push(
            MapArea::new(
                VirtAddr(sbss_with_stack as *const () as usize),
                VirtAddr(ebss as *const () as usize),
                MapType::Identical,
                MapPermission::R | MapPermission::W,
            ),
            None,
        );
        debug!("mapping physical memory");
        memory_set.push(
            MapArea::new(
                VirtAddr(ekernel as *const () as usize),
                VirtAddr(MEMORY_END),
                MapType::Identical,
                MapPermission::R | MapPermission::W,
            ),
            None,
        );
        debug!("mapping MMIO");
        for pair in crate::config::MMIO {
            memory_set.push(
                MapArea::new(
                    VirtAddr(pair.0),
                    VirtAddr(pair.0 + pair.1),
                    MapType::Identical,
                    MapPermission::R | MapPermission::W,
                ),
                None,
            );
        }
        memory_set
    }

    /// 包括 elf 中的各个段、trampoline、TrapContext 和用户栈
    /// 并返回用户堆底和入口点
    pub fn from_elf(elf_data: &[u8]) -> (Self, usize, usize) {
        let mut memory_set = Self::new_bare();
        // 映射跳板
        memory_set.map_trampoline();
        // 解析 elf，映射各个 loadable segment，权限根据段类型设置
        let elf = xmas_elf::ElfFile::new(elf_data).unwrap();
        let elf_header = elf.header;
        let magic = elf_header.pt1.magic;
        assert_eq!(magic, [0x7f, 0x45, 0x4c, 0x46], "invalid elf!");
        let ph_count = elf_header.pt2.ph_count();
        let mut max_end_vpn = VirtPageNum(0);
        for i in 0..ph_count {
            let ph = elf.program_header(i).unwrap();
            if ph.get_type().unwrap() == xmas_elf::program::Type::Load {
                let start_va: VirtAddr = VirtAddr(ph.virtual_addr() as usize);
                let end_va: VirtAddr = VirtAddr((ph.virtual_addr() + ph.mem_size()) as usize);
                let mut map_perm = MapPermission::U;
                let ph_flags = ph.flags();
                if ph_flags.is_read() {
                    map_perm |= MapPermission::R;
                }
                if ph_flags.is_write() {
                    map_perm |= MapPermission::W;
                }
                if ph_flags.is_execute() {
                    map_perm |= MapPermission::X;
                }
                let map_area = MapArea::new(start_va, end_va, MapType::Framed, map_perm);
                max_end_vpn = map_area.vpn_range.get_end();
                memory_set.push(
                    map_area,
                    Some(&elf.input[ph.offset() as usize..(ph.offset() + ph.file_size()) as usize]),
                );
            }
        }
        let max_end_va: VirtAddr = max_end_vpn.into();
        // 映射堆，位于用户空间底部的elf段之后，向上生长
        memory_set.push(
            MapArea::new(
                max_end_va,
                max_end_va,
                MapType::Framed,
                MapPermission::R | MapPermission::W | MapPermission::U,
            ),
            None,
        );
        info!(
            "内存映射完成，用户堆底地址 = {:#x}, 入口点 = {:#x}",
            max_end_va.0,
            elf.header.pt2.entry_point()
        );
        (
            memory_set,
            max_end_va.0,
            elf.header.pt2.entry_point() as usize,
        )
    }
    pub fn activate(&self) {
        let satp = self.page_table.token();
        unsafe {
            satp::write(Satp::from_bits(satp));
            asm!("sfence.vma");
        }
    }
    pub fn translate(&self, vpn: VirtPageNum) -> Option<PageTableEntry> {
        self.page_table.translate(vpn)
    }
    pub fn shrink_to(&mut self, start: VirtAddr, new_end: VirtAddr) -> KernelResult<()> {
        if let Some(area) = self
            .areas
            .iter_mut()
            .find(|area| area.vpn_range.get_start() == start.floor())
        {
            area.shrink_to(&mut self.page_table, new_end.ceil());
            Ok(())
        } else {
            Err(KernelError::ShrinkVirtAddrNotFound)
        }
    }
    pub fn append_to(&mut self, start: VirtAddr, new_end: VirtAddr) -> KernelResult<()> {
        if let Some(area) = self
            .areas
            .iter_mut()
            .find(|area| area.vpn_range.get_start() == start.floor())
        {
            area.append_to(&mut self.page_table, new_end.ceil())
        } else {
            Err(KernelError::AppendVirtAddrNotFound)
        }
    }
    pub fn mmap(&mut self, start: usize, end: usize, prot: usize) -> isize {
        let start_va = VirtAddr(start);
        let end_va = VirtAddr(end);
        if start >= TRAMPOLINE || end > TRAMPOLINE || start >= end {
            return -1;
        }
        if self
            .areas
            .iter()
            .any(|area| area.is_overlap(start_va, end_va))
        {
            return -1;
        }
        let permission = MapPermission::from_bits(prot as u8).unwrap() | MapPermission::U;
        self.insert_framed_area(start_va, end_va, permission);
        0
    }
    pub fn munmap(&mut self, start: usize, end: usize) -> isize {
        let start_va = VirtAddr(start);
        let end_va = VirtAddr(end);
        let start_vpn = start_va.floor();
        let end_vpn = end_va.ceil();
        loop {
            let mut target_idx = None;
            for (i, area) in self.areas.iter().enumerate() {
                if area.is_overlap(start_va, end_va) {
                    target_idx = Some(i);
                    break;
                }
            }
            if let Some(idx) = target_idx {
                let mut area = self.areas.remove(idx);

                // 1. 先把用户请求的这段 [start_vpn, end_vpn) 给 unmap 掉（物理回收）
                // 只有重叠的部分才需要 unmap
                let unmap_start = start_vpn.max(area.vpn_range.get_start());
                let unmap_end = end_vpn.min(area.vpn_range.get_end());

                for vpn in VPNRange::new(unmap_start, unmap_end) {
                    area.unmap_one(&mut self.page_table, vpn);
                }

                // 2. 逻辑切分：检查卸载后，原来的 area 是否还有“剩余”部分

                // 情况 A: 左侧有剩余 [original_start, start_vpn)
                if area.vpn_range.get_start() < start_vpn {
                    let left_end = start_vpn;
                    let mut left_area = MapArea {
                        vpn_range: VPNRange::new(area.vpn_range.get_start(), left_end),
                        data_frames: BTreeMap::new(),
                        map_type: area.map_type,
                        map_perm: area.map_perm,
                    };
                    // 将还在范围内的数据帧挪过去
                    let vpn_to_move: Vec<_> = area
                        .data_frames
                        .keys()
                        .filter(|&&vpn| vpn < left_end)
                        .cloned()
                        .collect();
                    for vpn in vpn_to_move {
                        if let Some(frame) = area.data_frames.remove(&vpn) {
                            left_area.data_frames.insert(vpn, frame);
                        }
                    }
                    self.areas.push(left_area);
                }

                // 情况 B: 右侧有剩余 [end_vpn, original_end)
                if area.vpn_range.get_end() > end_vpn {
                    let right_start = end_vpn;
                    let mut right_area = MapArea {
                        vpn_range: VPNRange::new(right_start, area.vpn_range.get_end()),
                        data_frames: BTreeMap::new(),
                        map_type: area.map_type,
                        map_perm: area.map_perm,
                    };
                    // 将还在范围内的数据帧挪过去
                    let vpn_to_move: Vec<_> = area
                        .data_frames
                        .keys()
                        .filter(|&&vpn| vpn >= right_start)
                        .cloned()
                        .collect();
                    for vpn in vpn_to_move {
                        if let Some(frame) = area.data_frames.remove(&vpn) {
                            right_area.data_frames.insert(vpn, frame);
                        }
                    }
                    self.areas.push(right_area);
                }
            } else {
                break;
            }
        }
        0
    }
}

/// map area structure, controls a contiguous piece of virtual memory
pub struct MapArea {
    vpn_range: VPNRange,
    data_frames: BTreeMap<VirtPageNum, FrameTracker>,
    map_type: MapType,
    map_perm: MapPermission,
}

impl MapArea {
    pub fn new(
        start_va: VirtAddr,
        end_va: VirtAddr,
        map_type: MapType,
        map_perm: MapPermission,
    ) -> Self {
        let start_vpn: VirtPageNum = start_va.floor();
        let end_vpn: VirtPageNum = end_va.ceil();
        Self {
            vpn_range: VPNRange::new(start_vpn, end_vpn),
            data_frames: BTreeMap::new(),
            map_type,
            map_perm,
        }
    }
    pub fn from_another(map_area: &MapArea) -> Self {
        Self {
            vpn_range: map_area.vpn_range,
            data_frames: BTreeMap::new(),
            map_type: map_area.map_type,
            map_perm: map_area.map_perm,
        }
    }
    pub fn map_one(&mut self, page_table: &mut PageTable, vpn: VirtPageNum) -> KernelResult<()> {
        let ppn: PhysPageNum;
        match self.map_type {
            MapType::Identical => {
                ppn = PhysPageNum(vpn.0);
            }
            MapType::Framed => {
                let frame = frame_alloc().ok_or(KernelError::FrameAllocFailed)?;
                ppn = frame.ppn;
                self.data_frames.insert(vpn, frame);
            }
        }
        let pte_flags = PTEFlags::from_bits(self.map_perm.bits()).unwrap();
        page_table.map(vpn, ppn, pte_flags);
        Ok(())
    }
    pub fn unmap_one(&mut self, page_table: &mut PageTable, vpn: VirtPageNum) {
        if self.map_type == MapType::Framed {
            self.data_frames.remove(&vpn);
        }
        page_table.unmap(vpn);
    }
    pub fn map(&mut self, page_table: &mut PageTable) {
        for vpn in self.vpn_range {
            self.map_one(page_table, vpn).unwrap();
        }
    }
    #[allow(unused)]
    pub fn unmap(&mut self, page_table: &mut PageTable) {
        for vpn in self.vpn_range {
            self.unmap_one(page_table, vpn);
        }
    }
    pub fn shrink_to(&mut self, page_table: &mut PageTable, new_end: VirtPageNum) {
        for vpn in VPNRange::new(new_end, self.vpn_range.get_end()) {
            self.unmap_one(page_table, vpn)
        }
        self.vpn_range = VPNRange::new(self.vpn_range.get_start(), new_end);
    }
    pub fn append_to(
        &mut self,
        page_table: &mut PageTable,
        new_end: VirtPageNum,
    ) -> KernelResult<()> {
        for vpn in VPNRange::new(self.vpn_range.get_end(), new_end) {
            self.map_one(page_table, vpn)?;
        }
        self.vpn_range = VPNRange::new(self.vpn_range.get_start(), new_end);
        Ok(())
    }
    /// data: start-aligned but maybe with shorter length
    /// assume that all frames were cleared before
    pub fn copy_data(&mut self, page_table: &PageTable, data: &[u8]) {
        assert_eq!(self.map_type, MapType::Framed);
        let mut start: usize = 0;
        let mut current_vpn = self.vpn_range.get_start();
        let len = data.len();
        loop {
            let src = &data[start..len.min(start + PAGE_SIZE)];
            let dst = &mut page_table
                .translate(current_vpn)
                .unwrap()
                .ppn()
                .get_bytes_array()[..src.len()];
            dst.copy_from_slice(src);
            start += PAGE_SIZE;
            if start >= len {
                break;
            }
            current_vpn.step();
        }
    }
    pub fn is_overlap(&self, start_va: VirtAddr, end_va: VirtAddr) -> bool {
        let start_vpn = start_va.floor();
        let end_vpn = end_va.ceil();
        !(end_vpn <= self.vpn_range.get_start() || start_vpn >= self.vpn_range.get_end())
    }
}

#[derive(Copy, Clone, PartialEq, Debug)]
/// map type for memory set: identical or framed
pub enum MapType {
    Identical,
    Framed,
}
bitflags::bitflags! {
    /// map permission corresponding to that in pte: `R W X U`
    #[derive(Clone, Copy)]
    pub struct MapPermission: u8 {
        const R = 1 << 1;
        const W = 1 << 2;
        const X = 1 << 3;
        const U = 1 << 4;
    }
}
