use core::{alloc::GlobalAlloc, cell::UnsafeCell, mem, ptr::NonNull};

use crate::syscall::sys_sbrk;

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

struct SimpleAllocator {
    head: UnsafeCell<Option<NonNull<FreeBlock>>>,
}

unsafe impl Sync for SimpleAllocator {}

impl SimpleAllocator {
    const fn new() -> Self {
        Self {
            head: UnsafeCell::new(None),
        }
    }

    #[inline]
    fn align_up(addr: usize, align: usize) -> usize {
        (addr + align - 1) & !(align - 1)
    }

    #[allow(unsafe_op_in_unsafe_fn)]
    unsafe fn insert_free_block(&self, addr: usize, size: usize) {
        if size < MIN_BLOCK_SIZE {
            return;
        }
        let head = self.head.get();
        let mut prev: Option<NonNull<FreeBlock>> = None;
        let mut curr = *head;

        while let Some(node) = curr {
            if (node.as_ptr() as usize) >= addr {
                break;
            }
            prev = curr;
            curr = node.as_ref().next;
        }

        let new_node = addr as *mut FreeBlock;
        new_node.write(FreeBlock { size, next: curr });
        let mut new_node_ptr = NonNull::new_unchecked(new_node);

        if let Some(mut prev_node) = prev {
            prev_node.as_mut().next = Some(new_node_ptr);
        } else {
            *head = Some(new_node_ptr);
        }

        // Merge with next contiguous free block.
        if let Some(next_node) = new_node_ptr.as_ref().next {
            let new_end = addr + new_node_ptr.as_ref().size;
            let next_addr = next_node.as_ptr() as usize;
            if new_end == next_addr {
                let next_next = next_node.as_ref().next;
                let merged_size = new_node_ptr.as_ref().size + next_node.as_ref().size;
                new_node_ptr.as_mut().size = merged_size;
                new_node_ptr.as_mut().next = next_next;
            }
        }

        // Merge with previous contiguous free block.
        if let Some(mut prev_node) = prev {
            let prev_addr = prev_node.as_ptr() as usize;
            let prev_end = prev_addr + prev_node.as_ref().size;
            if prev_end == addr {
                let merged_size = prev_node.as_ref().size + new_node_ptr.as_ref().size;
                prev_node.as_mut().size = merged_size;
                prev_node.as_mut().next = new_node_ptr.as_ref().next;
            }
        }
    }

    #[allow(unsafe_op_in_unsafe_fn)]
    unsafe fn request_more_and_insert(&self, need_size: usize) -> bool {
        let request = Self::align_up(need_size.max(PAGE_SIZE), PAGE_SIZE);
        let old_brk = sys_sbrk(request as i32);
        if old_brk == -1 {
            return false;
        }
        self.insert_free_block(old_brk as usize, request);
        true
    }
}

unsafe impl GlobalAlloc for SimpleAllocator {
    #[allow(unsafe_op_in_unsafe_fn)]
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        let align = layout.align().max(mem::align_of::<AllocHeader>());
        let need_payload = layout.size().max(1);
        let need_total = (mem::size_of::<AllocHeader>() + need_payload).max(MIN_BLOCK_SIZE);

        loop {
            let head = self.head.get();
            let mut prev: Option<NonNull<FreeBlock>> = None;
            let mut curr = *head;

            while let Some(node) = curr {
                let block_start = node.as_ptr() as usize;
                let block_size = node.as_ref().size;
                let block_end = block_start + block_size;

                let payload = Self::align_up(block_start + mem::size_of::<AllocHeader>(), align);
                let alloc_start = payload - mem::size_of::<AllocHeader>();
                let mut alloc_end = alloc_start.saturating_add(need_total);

                if alloc_end <= block_end {
                    let prefix = alloc_start - block_start;
                    if prefix != 0 && prefix < MIN_BLOCK_SIZE {
                        prev = curr;
                        curr = node.as_ref().next;
                        continue;
                    }

                    let suffix = block_end - alloc_end;
                    if suffix != 0 && suffix < MIN_BLOCK_SIZE {
                        alloc_end = block_end;
                    }

                    let next = node.as_ref().next;
                    if let Some(mut prev_node) = prev {
                        prev_node.as_mut().next = next;
                    } else {
                        *head = next;
                    }

                    let alloc_size = alloc_end - alloc_start;
                    let header = alloc_start as *mut AllocHeader;
                    header.write(AllocHeader { size: alloc_size });

                    if prefix >= MIN_BLOCK_SIZE {
                        self.insert_free_block(block_start, prefix);
                    }
                    let suffix_final = block_end - alloc_end;
                    if suffix_final >= MIN_BLOCK_SIZE {
                        self.insert_free_block(alloc_end, suffix_final);
                    }

                    return payload as *mut u8;
                }

                prev = curr;
                curr = node.as_ref().next;
            }

            if !self.request_more_and_insert(need_total + align) {
                return core::ptr::null_mut();
            }
        }
    }

    #[allow(unsafe_op_in_unsafe_fn)]
    unsafe fn dealloc(&self, ptr: *mut u8, _layout: core::alloc::Layout) {
        if ptr.is_null() {
            return;
        }
        let header_addr = (ptr as usize) - mem::size_of::<AllocHeader>();
        let header = &*(header_addr as *const AllocHeader);
        self.insert_free_block(header_addr, header.size);
    }
}

#[global_allocator]
static ALLOCATOR: SimpleAllocator = SimpleAllocator::new();
