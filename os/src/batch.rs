//! 批处理

use log::info;

use crate::{sync::SyncRefCell, trap::TrapContext};
use core::arch::asm;

const USER_STACK_SIZE: usize = 4096 * 2;
const KERNEL_STACK_SIZE: usize = 4096 * 2;
const MAX_APP_NUM: usize = 16;
const APP_BASE_ADDRESS: usize = 0x80400000;
const APP_SIZE_LIMIT: usize = 0x20000;

#[repr(align(4096))]
struct KernelStack {
    data: [u8; KERNEL_STACK_SIZE],
}

#[repr(align(4096))]
struct UserStack {
    data: [u8; USER_STACK_SIZE],
}

static KERNEL_STACK: KernelStack = KernelStack {
    data: [0; KERNEL_STACK_SIZE],
};
static USER_STACK: UserStack = UserStack {
    data: [0; USER_STACK_SIZE],
};

impl KernelStack {
    fn get_sp(&self) -> usize {
        self.data.as_ptr() as usize + KERNEL_STACK_SIZE
    }
    pub fn push_context(&self, cx: TrapContext) -> &'static mut TrapContext {
        let cx_ptr = (self.get_sp() - core::mem::size_of::<TrapContext>()) as *mut TrapContext;
        unsafe {
            *cx_ptr = cx;
        }
        unsafe { cx_ptr.as_mut().unwrap() }
    }
}

impl UserStack {
    fn get_sp(&self) -> usize {
        self.data.as_ptr() as usize + USER_STACK_SIZE
    }
}

struct AppManager {
    num_app: usize,
    current_app: usize,
    app_start: [usize; MAX_APP_NUM + 1],
}

impl AppManager {
    pub fn print_app_info(&self) {
        info!("[kernel] 应用数量 = {}", self.num_app);
        for i in 0..self.num_app {
            info!(
                "[kernel] 应用 {} 的地址范围是 [{:#x}, {:#x})",
                i,
                self.app_start[i],
                self.app_start[i + 1]
            );
        }
    }

    fn load_app(&self, app_id: usize) {
        if app_id >= self.num_app {
            info!("[kernel] 所有应用执行完毕！");
            crate::sbi::shutdown()
        }
        info!("[kernel] 正在加载应用 {}", app_id);
        unsafe {
            // 清空指令内存并加载下一个应用的代码到指令内存
            core::slice::from_raw_parts_mut(APP_BASE_ADDRESS as *mut u8, APP_SIZE_LIMIT).fill(0);
            let app_src = core::slice::from_raw_parts(
                self.app_start[app_id] as *const u8,
                self.app_start[app_id + 1] - self.app_start[app_id],
            );
            let app_dst =
                core::slice::from_raw_parts_mut(APP_BASE_ADDRESS as *mut u8, app_src.len());
            app_dst.copy_from_slice(app_src);
            // CPU一般会认为指令内存是只读的
            // 因此需要执行fence.i指令来刷新指令缓存
            // 使得新加载的指令能够被CPU正确识别
            asm!("fence.i");
        }
    }

    pub fn get_current_app(&self) -> usize {
        self.current_app
    }

    pub fn move_to_next_app(&mut self) {
        self.current_app += 1;
    }
}

lazy_static::lazy_static! {
    static ref APP_MANAGER: SyncRefCell<AppManager> = unsafe {
        SyncRefCell::new({
            unsafe extern "C" {
                safe fn _num_app();
            }
            let num_app_ptr = _num_app as *const usize;
            let num_app = num_app_ptr.read_volatile();
            let mut app_start = [0usize; MAX_APP_NUM + 1];
            let app_start_raw: &[usize] =
                core::slice::from_raw_parts(num_app_ptr.add(1), num_app + 1);
            app_start[..=num_app].copy_from_slice(app_start_raw);
            AppManager {
                num_app,
                current_app: 0,
                app_start,
            }
        })
    };
}

/// 初始化批处理系统
pub fn init() {
    print_app_info();
}

/// 打印应用信息
pub fn print_app_info() {
    APP_MANAGER.exclusive_access().print_app_info();
}

/// 运行下一个应用
pub fn run_next_app() -> ! {
    let mut app_manager = APP_MANAGER.exclusive_access();
    let current_app = app_manager.get_current_app();
    app_manager.load_app(current_app);
    app_manager.move_to_next_app();
    drop(app_manager);

    // 伪造 TrapContext 并切换到用户态运行应用程序
    unsafe extern "C" {
        unsafe fn __restore(cx_addr: usize);
    }
    let kernel_stack_top = KERNEL_STACK.push_context(TrapContext::app_init_context(
        APP_BASE_ADDRESS,
        USER_STACK.get_sp(),
    )) as *const _ as usize;
    unsafe {
        __restore(kernel_stack_top);
    }
    panic!("Unreachable in batch::run_current_app!");
}
