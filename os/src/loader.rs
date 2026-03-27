//! 将应用程序数据加载到内存中

use alloc::vec::Vec;

/// 获取应用程序的总数
pub fn get_num_app() -> usize {
    unsafe extern "C" {
        safe fn _num_app();
    }
    unsafe { (_num_app as *const usize).read_volatile() }
}

fn get_app_data(app_id: usize) -> &'static [u8] {
    unsafe extern "C" {
        safe fn _num_app();
    }
    let num_app_ptr = _num_app as *const usize;
    let num_app = get_num_app();
    let app_start = unsafe { core::slice::from_raw_parts(num_app_ptr.add(1), num_app + 1) };
    assert!(app_id < num_app);
    unsafe {
        core::slice::from_raw_parts(
            app_start[app_id] as *const u8,
            app_start[app_id + 1] - app_start[app_id],
        )
    }
}

/// 根据应用程序名称获取应用程序数据
pub fn get_app_data_by_name(name: &str) -> Option<&'static [u8]> {
    let num_app = get_num_app();
    (0..num_app)
        .find(|&i| APP_NAMES[i] == name)
        .map(get_app_data)
}

pub fn list_apps() {
    println!("可用的应用程序：");
    for (i, name) in APP_NAMES
        .iter()
        .filter(|&&name| name != "initproc")
        .enumerate()
    {
        println!("{}: {}", i, name);
    }
    println!("输入 '<app_name>' 来运行一个应用程序");
}

lazy_static::lazy_static! {
    static ref APP_NAMES: Vec<&'static str> = {
        let num_app = get_num_app();
        unsafe extern "C" {
            fn _app_names();
        }
        let mut start = _app_names as *const u8;
        let mut v = Vec::new();
        unsafe {
            for _ in 0..num_app {
                let mut end = start;
                while end.read_volatile() != b'\0' {
                    end = end.add(1);
                }
                let slice = core::slice::from_raw_parts(start, end as usize - start as usize);
                let str = core::str::from_utf8(slice).unwrap();
                v.push(str);
                start = end.add(1);
            }
        }
        v
    };
}
