use core::{
    alloc::{GlobalAlloc, Layout},
    mem,
    ptr::NonNull,
};

use crate::{sync::SpinLock, syscall::sys_sbrk};

const PAGE_SIZE: usize = 4096;
const MIN_BLOCK_SIZE: usize = mem::size_of::<FreeBlock>();

#[repr(C)]
struct FreeBlock {
    size: usize,
    next: Option<NonNull<FreeBlock>>,
}

#[repr(C)]
struct AllocHeader {
    size: usize,
}

pub struct SimpleAllocator {
    head: SpinLock<Option<NonNull<FreeBlock>>>,
}

impl SimpleAllocator {
    pub const fn new() -> Self {
        Self {
            head: SpinLock::new(None),
        }
    }

    #[inline]
    fn align_up(addr: usize, align: usize) -> usize {
        (addr + align - 1) & !(align - 1)
    }

    /// 全局有序插入并自动合并相邻块 (仅在 dealloc 和 sbrk 时调用)
    #[allow(unsafe_op_in_unsafe_fn)]
    unsafe fn insert_free_block(head: &mut Option<NonNull<FreeBlock>>, addr: usize, size: usize) {
        if size < MIN_BLOCK_SIZE {
            return;
        }
        let mut prev: Option<NonNull<FreeBlock>> = None;
        let mut curr = *head;

        // 寻找插入点，保持地址单调递增
        while let Some(node) = curr {
            if (node.as_ptr() as usize) >= addr {
                break;
            }
            prev = curr;
            curr = node.as_ref().next;
        }

        let new_node_ptr = addr as *mut FreeBlock;
        new_node_ptr.write(FreeBlock { size, next: curr });
        let mut new_node = NonNull::new_unchecked(new_node_ptr);

        if let Some(mut prev_node) = prev {
            prev_node.as_mut().next = Some(new_node);
        } else {
            *head = Some(new_node);
        }

        // 1. 尝试与后一个块合并
        if let Some(next_node) = new_node.as_ref().next
            && addr + new_node.as_ref().size == next_node.as_ptr() as usize
        {
            new_node.as_mut().size += next_node.as_ref().size;
            new_node.as_mut().next = next_node.as_ref().next;
        }

        // 2. 尝试与前一个块合并
        if let Some(mut prev_node) = prev
            && (prev_node.as_ptr() as usize) + prev_node.as_ref().size == addr
        {
            prev_node.as_mut().size += new_node.as_ref().size;
            prev_node.as_mut().next = new_node.as_ref().next;
        }
    }

    #[allow(unsafe_op_in_unsafe_fn)]
    unsafe fn request_more_and_insert(
        head: &mut Option<NonNull<FreeBlock>>,
        need_size: usize,
    ) -> bool {
        let request = Self::align_up(need_size.max(PAGE_SIZE), PAGE_SIZE);
        let old_brk = sys_sbrk(request as i32);
        if old_brk == -1 {
            return false;
        }
        Self::insert_free_block(head, old_brk as usize, request);
        true
    }
}

unsafe impl GlobalAlloc for SimpleAllocator {
    #[allow(unsafe_op_in_unsafe_fn)]
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let mut head = self.head.lock();
        let align = layout.align().max(mem::align_of::<AllocHeader>());
        let need_payload = layout.size().max(1);

        loop {
            let mut prev: Option<NonNull<FreeBlock>> = None;
            let mut curr = *head;

            while let Some(node) = curr {
                let block_start = node.as_ptr() as usize;
                let block_size = node.as_ref().size;
                let block_end = block_start + block_size;

                // 计算 payload 地址，确保其满足对齐要求
                let mut payload =
                    Self::align_up(block_start + mem::size_of::<AllocHeader>(), align);
                let mut alloc_start = payload - mem::size_of::<AllocHeader>();

                // 核心逻辑 1：动态对齐补偿
                // 如果前缀空间不足以塞下一个完整的 FreeBlock，就向后移动一个 align 步长
                while alloc_start > block_start && (alloc_start - block_start) < MIN_BLOCK_SIZE {
                    payload += align;
                    alloc_start = payload - mem::size_of::<AllocHeader>();
                }

                let mut alloc_end =
                    alloc_start.saturating_add(mem::size_of::<AllocHeader>() + need_payload);

                // 如果当前块容量足够
                if alloc_end <= block_end {
                    let prefix = alloc_start - block_start;
                    let mut suffix = block_end - alloc_end;

                    // 核心逻辑 2：尾部碎片吸收
                    if suffix > 0 && suffix < MIN_BLOCK_SIZE {
                        alloc_end = block_end; // 将尾部无法独立成块的碎片直接送给本次分配
                        suffix = 0;
                    }

                    // 核心逻辑 3：O(1) 原地切割与链表修补 (拒绝二次遍历)
                    let mut replacement_head = node.as_ref().next;

                    // 处理后缀：把它变成一个新的节点，指向原先的 next
                    if suffix >= MIN_BLOCK_SIZE {
                        let suffix_ptr = alloc_end as *mut FreeBlock;
                        suffix_ptr.write(FreeBlock {
                            size: suffix,
                            next: replacement_head,
                        });
                        replacement_head = Some(NonNull::new_unchecked(suffix_ptr));
                    }

                    // 处理前缀：把它变成一个新的节点，指向后缀（或者原先的 next）
                    if prefix >= MIN_BLOCK_SIZE {
                        let prefix_ptr = block_start as *mut FreeBlock;
                        prefix_ptr.write(FreeBlock {
                            size: prefix,
                            next: replacement_head,
                        });
                        replacement_head = Some(NonNull::new_unchecked(prefix_ptr));
                    }

                    // 将新生成的链表头接到前驱节点上
                    if let Some(mut prev_node) = prev {
                        prev_node.as_mut().next = replacement_head;
                    } else {
                        *head = replacement_head;
                    }

                    // 写入 AllocHeader
                    let alloc_size = alloc_end - alloc_start;
                    let header = alloc_start as *mut AllocHeader;
                    header.write(AllocHeader { size: alloc_size });

                    return payload as *mut u8;
                }

                prev = curr;
                curr = node.as_ref().next;
            }

            // 如果遍历完都没找到，向系统申请更多内存
            if !Self::request_more_and_insert(
                &mut head,
                need_payload + mem::size_of::<AllocHeader>() + align,
            ) {
                return core::ptr::null_mut(); // OOM
            }
        }
    }

    #[allow(unsafe_op_in_unsafe_fn)]
    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
        if ptr.is_null() {
            return;
        }
        let mut head = self.head.lock(); // 注意这里加上 mut
        let header_addr = (ptr as usize) - mem::size_of::<AllocHeader>();
        let header = &*(header_addr as *const AllocHeader);

        // dealloc 时才进行全局搜索并回收合并
        Self::insert_free_block(&mut head, header_addr, header.size);
    }
}

// 确保在 lib.rs 或 main.rs 中声明：
#[global_allocator]
static ALLOCATOR: SimpleAllocator = SimpleAllocator::new();
