use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::arch::asm;
use lazy_static::*;
use log::info;
use riscv::register::satp::{self, Satp};

use crate::config::{
    MEMORY_END, PAGE_SIZE, TRAMPOLINE, TRAP_CONTEXT, USER_STACK_BOTTOM, USER_STACK_TOP,
};
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
        info!(
            ".text [{:#x}, {:#x})",
            stext as *const () as usize, etext as *const () as usize
        );
        info!(
            ".rodata [{:#x}, {:#x})",
            srodata as *const () as usize, erodata as *const () as usize
        );
        info!(
            ".data [{:#x}, {:#x})",
            sdata as *const () as usize, edata as *const () as usize
        );
        info!(
            ".bss [{:#x}, {:#x})",
            sbss_with_stack as *const () as usize, ebss as *const () as usize
        );
        info!("mapping .text section");
        memory_set.push(
            MapArea::new(
                VirtAddr(stext as *const () as usize),
                VirtAddr(etext as *const () as usize),
                MapType::Identical,
                MapPermission::R | MapPermission::X,
            ),
            None,
        );
        info!("mapping .rodata section");
        memory_set.push(
            MapArea::new(
                VirtAddr(srodata as *const () as usize),
                VirtAddr(erodata as *const () as usize),
                MapType::Identical,
                MapPermission::R,
            ),
            None,
        );
        info!("mapping .data section");
        memory_set.push(
            MapArea::new(
                VirtAddr(sdata as *const () as usize),
                VirtAddr(edata as *const () as usize),
                MapType::Identical,
                MapPermission::R | MapPermission::W,
            ),
            None,
        );
        info!("mapping .bss section");
        memory_set.push(
            MapArea::new(
                VirtAddr(sbss_with_stack as *const () as usize),
                VirtAddr(ebss as *const () as usize),
                MapType::Identical,
                MapPermission::R | MapPermission::W,
            ),
            None,
        );
        info!("mapping physical memory");
        memory_set.push(
            MapArea::new(
                VirtAddr(ekernel as *const () as usize),
                VirtAddr(MEMORY_END),
                MapType::Identical,
                MapPermission::R | MapPermission::W,
            ),
            None,
        );
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
        // 映射用户栈，位于用户空间顶部TrapContext下，向下生长
        info!(
            "mapping user stack [{:#x}, {:#x})",
            USER_STACK_BOTTOM, USER_STACK_TOP
        );
        memory_set.push(
            MapArea::new(
                VirtAddr(USER_STACK_BOTTOM),
                VirtAddr(USER_STACK_TOP),
                MapType::Framed,
                MapPermission::R | MapPermission::W | MapPermission::U,
            ),
            None,
        );

        let max_end_va: VirtAddr = max_end_vpn.into();
        // 映射堆，位于用户空间底部的elf段之后，向上生长
        info!("mapping heap [{:#x}, {:#x})", max_end_va.0, max_end_va.0);
        memory_set.push(
            MapArea::new(
                max_end_va,
                max_end_va,
                MapType::Framed,
                MapPermission::R | MapPermission::W | MapPermission::U,
            ),
            None,
        );
        // 映射 TrapContext，位于用户空间顶部跳板下，用户栈上，权限为 R/W
        memory_set.push(
            MapArea::new(
                VirtAddr(TRAP_CONTEXT),
                VirtAddr(TRAMPOLINE),
                MapType::Framed,
                MapPermission::R | MapPermission::W,
            ),
            None,
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
    pub fn shrink_to(&mut self, start: VirtAddr, new_end: VirtAddr) -> bool {
        if let Some(area) = self
            .areas
            .iter_mut()
            .find(|area| area.vpn_range.get_start() == start.floor())
        {
            area.shrink_to(&mut self.page_table, new_end.ceil());
            true
        } else {
            false
        }
    }
    pub fn append_to(&mut self, start: VirtAddr, new_end: VirtAddr) -> bool {
        if let Some(area) = self
            .areas
            .iter_mut()
            .find(|area| area.vpn_range.get_start() == start.floor())
        {
            area.append_to(&mut self.page_table, new_end.ceil());
            true
        } else {
            false
        }
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
    pub fn map_one(&mut self, page_table: &mut PageTable, vpn: VirtPageNum) {
        let ppn: PhysPageNum;
        match self.map_type {
            MapType::Identical => {
                ppn = PhysPageNum(vpn.0);
            }
            MapType::Framed => {
                let frame = frame_alloc().unwrap();
                ppn = frame.ppn;
                self.data_frames.insert(vpn, frame);
            }
        }
        let pte_flags = PTEFlags::from_bits(self.map_perm.bits()).unwrap();
        page_table.map(vpn, ppn, pte_flags);
    }
    pub fn unmap_one(&mut self, page_table: &mut PageTable, vpn: VirtPageNum) {
        if self.map_type == MapType::Framed {
            self.data_frames.remove(&vpn);
        }
        page_table.unmap(vpn);
    }
    pub fn map(&mut self, page_table: &mut PageTable) {
        for vpn in self.vpn_range {
            self.map_one(page_table, vpn);
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
    pub fn append_to(&mut self, page_table: &mut PageTable, new_end: VirtPageNum) {
        for vpn in VPNRange::new(self.vpn_range.get_end(), new_end) {
            self.map_one(page_table, vpn)
        }
        self.vpn_range = VPNRange::new(self.vpn_range.get_start(), new_end);
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
}

#[derive(Copy, Clone, PartialEq, Debug)]
/// map type for memory set: identical or framed
pub enum MapType {
    Identical,
    Framed,
}
bitflags::bitflags! {
    /// map permission corresponding to that in pte: `R W X U`
    pub struct MapPermission: u8 {
        const R = 1 << 1;
        const W = 1 << 2;
        const X = 1 << 3;
        const U = 1 << 4;
    }
}
