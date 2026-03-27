use core::{alloc::GlobalAlloc, ptr::NonNull};

use crate::syscall::sys_sbrk;

struct FreeBlock {
    size: usize,
    next: Option<NonNull<FreeBlock>>,
}

struct SimpleAllocator {
    head: core::cell::UnsafeCell<Option<NonNull<FreeBlock>>>,
}

unsafe impl Sync for SimpleAllocator {}

impl SimpleAllocator {
    const fn new() -> Self {
        Self {
            head: core::cell::UnsafeCell::new(None),
        }
    }

    unsafe fn request_memory(&self, size: usize) -> *mut u8 {
        let page_size = 4096;
        let alloc_size = (size + page_size - 1) & !(page_size - 1);

        let old_break = sys_sbrk(alloc_size as i32);
        if old_break == -1 {
            core::ptr::null_mut()
        } else {
            old_break as *mut u8
        }
    }
}

unsafe impl GlobalAlloc for SimpleAllocator {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        unsafe {
            let size = layout.size().max(core::mem::size_of::<FreeBlock>());
            let align = layout.align();

            let head_ptr = self.head.get();
            let mut curr = *head_ptr;
            let mut prev: Option<NonNull<FreeBlock>> = None;

            while let Some(node) = curr {
                let addr = node.as_ptr() as usize;
                let aligned_addr = (addr + align - 1) & !(align - 1);
                if node.as_ref().size >= size + (aligned_addr - addr) {
                    if let Some(mut p) = prev {
                        p.as_mut().next = node.as_ref().next;
                    } else {
                        *head_ptr = node.as_ref().next;
                    }
                    return aligned_addr as *mut u8;
                }
                prev = curr;
                curr = node.as_ref().next;
            }
            let new_mem = self.request_memory(size + align);
            if new_mem.is_null() {
                core::ptr::null_mut()
            } else {
                let aligned_addr = (new_mem as usize + align - 1) & !(align - 1);
                aligned_addr as *mut u8
            }
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: core::alloc::Layout) {
        unsafe {
            let new_node_ptr = ptr as *mut FreeBlock;
            let new_node = FreeBlock {
                size: layout.size(),
                next: *self.head.get(),
            };
            new_node_ptr.write(new_node);
            *self.head.get() = Some(NonNull::new_unchecked(new_node_ptr));
        }
    }
}

#[global_allocator]
static ALLOCATOR: SimpleAllocator = SimpleAllocator::new();
