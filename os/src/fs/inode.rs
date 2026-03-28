//! Inode
use crate::drivers::block::BLOCK_DEVICE;
use alloc::{string::String, sync::Arc, vec::Vec};
use easy_fs::{EasyFileSystem, Inode};

use crate::{fs::File, sync::SyncRefCell};

pub struct OSInode {
    readable: bool,
    writeable: bool,
    inner: SyncRefCell<OSInodeInner>,
}

impl OSInode {
    /// 创建一个新的 OSInode
    pub fn new(readable: bool, writeable: bool, inode: Arc<Inode>) -> Self {
        Self {
            readable,
            writeable,
            inner: unsafe { SyncRefCell::new(OSInodeInner { offset: 0, inode }) },
        }
    }
    /// 读取文件内容到一个 Vec<u8> 中
    pub fn read_all(&self) -> Vec<u8> {
        let mut inner = self.inner.exclusive_access();
        let mut buffer = [0u8; 512];
        let mut v: Vec<u8> = Vec::new();
        loop {
            let len = inner.inode.read_at(inner.offset, &mut buffer);
            if len == 0 {
                break;
            }
            inner.offset += len;
            v.extend_from_slice(&buffer[..len]);
        }
        v
    }
}

struct OSInodeInner {
    offset: usize,
    inode: Arc<Inode>,
}

impl File for OSInode {
    fn readable(&self) -> bool {
        self.readable
    }

    fn writeable(&self) -> bool {
        self.writeable
    }

    fn read(&self, buf: crate::mem::UserBuffer) -> isize {
        let mut inner = self.inner.exclusive_access();
        let mut total_read_size = 0;
        for mut buf in buf.buf {
            let read_size = inner.inode.read_at(inner.offset, buf);
            if read_size == 0 {
                break;
            }
            inner.offset += read_size;
            total_read_size += read_size;
        }
        total_read_size as isize
    }

    fn write(&self, buf: crate::mem::UserBuffer) -> isize {
        let mut inner = self.inner.exclusive_access();
        let mut total_write_size = 0;
        for buf in buf.buf {
            let write_size = inner.inode.write_at(inner.offset, buf);
            assert_eq!(write_size, buf.len());
            inner.offset += write_size;
            total_write_size += write_size;
        }
        total_write_size as isize
    }
}

lazy_static::lazy_static! {
    pub static ref ROOT_INODE: Arc<Inode> = {
        let efs = easy_fs::EasyFileSystem::open(BLOCK_DEVICE.clone());
        Arc::new(EasyFileSystem::root_inode(&efs))
    };
}

pub fn list_apps() {
    println!("/**** APPS ****");
    for app in ROOT_INODE.ls() {
        println!("{}", app);
    }
    println!("**************/")
}

bitflags::bitflags! {
    pub struct OpenFlags: u32 {
        const RDONLY = 0;
        const WRONLY = 1 << 0;
        const RDWR = 1 << 1;
        const CREATE = 1 << 9;
        const TRUNC = 1 << 10;
        const DIRECTORY = 1 << 11;
    }
}

impl OpenFlags {
    pub fn read_write(&self) -> (bool, bool) {
        if self.is_empty() {
            (true, false)
        } else if self.contains(Self::WRONLY) {
            (false, true)
        } else {
            (true, true)
        }
    }
}

fn path_components(path: &str) -> Vec<&str> {
    path.split('/')
        .filter(|part| !part.is_empty() && *part != ".")
        .collect()
}

fn resolve_path(path: &str) -> Option<Arc<Inode>> {
    let parts = path_components(path);
    if parts.is_empty() {
        return Some(ROOT_INODE.clone());
    }
    let mut current = ROOT_INODE.clone();
    for part in parts {
        let next = current.find(part)?;
        current = next;
    }
    Some(current)
}

fn resolve_parent_dir(path: &str, create_intermediate: bool) -> Option<(Arc<Inode>, String)> {
    let mut parts = path_components(path);
    if parts.is_empty() {
        return None;
    }
    let file_name = String::from(parts.pop().unwrap());
    let mut current = ROOT_INODE.clone();
    for part in parts {
        if let Some(next) = current.find(part) {
            if !next.is_dir() {
                return None;
            }
            current = next;
        } else if create_intermediate {
            current = current.create_dir(part)?;
        } else {
            return None;
        }
    }
    Some((current, file_name))
}

pub fn open_file(name: &str, flags: OpenFlags) -> Option<Arc<OSInode>> {
    let (readable, writable) = flags.read_write();
    if flags.contains(OpenFlags::CREATE) {
        let (parent_dir, base_name) = resolve_parent_dir(name, true)?;
        if base_name.is_empty() {
            return None;
        }
        let want_dir = flags.contains(OpenFlags::DIRECTORY);
        if let Some(inode) = parent_dir.find(base_name.as_str()) {
            if want_dir && !inode.is_dir() {
                return None;
            }
            if !want_dir && inode.is_dir() {
                return None;
            }
            if !want_dir {
                inode.clear();
            }
            Some(Arc::new(OSInode::new(readable, writable, inode)))
        } else {
            let inode = if want_dir {
                parent_dir.create_dir(base_name.as_str())
            } else {
                parent_dir.create(base_name.as_str())
            };
            inode.map(|inode| Arc::new(OSInode::new(readable, writable, inode)))
        }
    } else {
        resolve_path(name).and_then(|inode| {
            if flags.contains(OpenFlags::DIRECTORY) && !inode.is_dir() {
                return None;
            }
            if !flags.contains(OpenFlags::DIRECTORY) && inode.is_dir() {
                return None;
            }
            if flags.contains(OpenFlags::TRUNC) && inode.is_file() {
                inode.clear();
            }
            Some(Arc::new(OSInode::new(readable, writable, inode)))
        })
    }
}
