//! 文件系统
#![allow(unused)]

use crate::mem::UserBuffer;
pub mod inode;
pub mod stdio;

pub trait File: Send + Sync {
    fn readable(&self) -> bool;
    fn writeable(&self) -> bool;
    fn read(&self, buf: UserBuffer) -> isize;
    fn write(&self, buf: UserBuffer) -> isize;
}
