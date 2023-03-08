#![feature(let_chains)]
#![no_std]
extern crate alloc;

use crate::fstype::MyBuffer;
use alloc::string::String;
use alloc::sync::Arc;
use core::fmt::{Debug, Formatter};
use fatfs::{DefaultTimeProvider, Dir, LossyOemCpConverter};
use rvfs::inode::Inode;
use rvfs::superblock::{DataOps, Device};
use spin::Mutex;

pub mod file;
pub mod fstype;
pub mod inode;

type FatDir = Dir<MyBuffer, DefaultTimeProvider, LossyOemCpConverter>;
/// Description:
///
/// Because the fatfs dont support inode,so we need save some information in inode.
/// According to the information in inode,we can get the find the file in fatfs.
/// The information include the file name because original filesystem's inode include the inode number
/// that can identify the file uniquely but fatfs dont have inode number.
pub struct FatInode {
    // parent
    pub parent: Arc<Mutex<FatDir>>,
    // self: if the file is a directory,then the self is the directory's DIR struct.
    pub current: FatInodeType,
}

pub enum FatInodeType {
    Dir(Arc<Mutex<FatDir>>),
    File(String),
}

impl FatInode {
    pub fn new(parent: Arc<Mutex<FatDir>>, current: FatInodeType) -> FatInode {
        Self { parent, current }
    }
}

impl Debug for FatInode {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        let current = &self.current;
        match current {
            FatInodeType::Dir(_) => f.write_str("FatInode::Dir"),
            FatInodeType::File(_) => f.write_str("FatInode::File"),
        }
    }
}

impl DataOps for FatInode {
    fn device(&self, _name: &str) -> Option<Arc<dyn Device>> {
        None
    }
    fn data(&self) -> *const u8 {
        self as *const Self as *const u8
    }
}

fn get_fat_data(inode: Arc<Inode>) -> &'static mut FatInode {
    let inode_inner = inode.access_inner();
    let data = inode_inner.data.as_ref().unwrap();
    unsafe { &mut *(data.data() as *mut FatInode) }
}
