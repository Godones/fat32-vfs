#![allow(unused)]
use crate::file::{FAT_DENTRY_OPS, FAT_DIR_FILE_OPS};
use crate::inode::FAT_INODE_DIR_OPS;
use crate::{FatDir, FatInode, FatInodeType};
use alloc::boxed::Box;
use alloc::string::ToString;
use alloc::sync::{Arc, Weak};
use fatfs::{DefaultTimeProvider, Dir, IoBase, LossyOemCpConverter, Read, Seek, SeekFrom, Write};
use fscommon::BufStream;
use rvfs::dentry::{DirEntry, DirFlags};
use rvfs::inode::{simple_statfs, Inode, InodeMode};
use rvfs::mount::MountFlags;
use rvfs::superblock::{
    DataOps, Device, FileSystemAttr, FileSystemType, SuperBlock, SuperBlockInner, SuperBlockOps,
};
use rvfs::{ddebug, iinfo, StrResult};
use spin::Mutex;

pub struct FatDevice {
    pos: i64,
    device_file: Arc<dyn Device>,
}
impl FatDevice {
    pub fn new(device: Arc<dyn Device>) -> Self {
        Self {
            pos: 0,
            device_file: device,
        }
    }
}
impl core2::io::Read for FatDevice {
    fn read(&mut self, buf: &mut [u8]) -> core2::io::Result<usize> {
        let len = self
            .device_file
            .read(buf, self.pos as usize)
            .map_err(|x| core2::io::Error::new(core2::io::ErrorKind::Other, "other"))?;
        self.pos += len as i64;
        Ok(len)
    }
}
impl core2::io::Write for FatDevice {
    fn write(&mut self, buf: &[u8]) -> core2::io::Result<usize> {
        let len = self
            .device_file
            .write(buf, self.pos as usize)
            .map_err(|x| core2::io::Error::new(core2::io::ErrorKind::Other, "other"))?;
        self.pos += len as i64;
        Ok(len)
    }
    fn flush(&mut self) -> core2::io::Result<()> {
        Ok(())
    }
}

impl core2::io::Seek for FatDevice {
    fn seek(&mut self, pos: core2::io::SeekFrom) -> core2::io::Result<u64> {
        let pos = match pos {
            core2::io::SeekFrom::Start(pos) => pos as i64,
            core2::io::SeekFrom::End(pos) => self.device_file.size() as i64 + pos,
            core2::io::SeekFrom::Current(pos) => self.pos + pos,
        };
        if pos < 0 {
            return Err(core2::io::Error::new(
                core2::io::ErrorKind::Other,
                "seek error",
            ));
        }
        self.pos = pos;
        Ok(pos as u64)
    }
}

pub struct MyBuffer {
    buf: BufStream<FatDevice>,
}

impl IoBase for MyBuffer {
    type Error = ();
}

impl Write for MyBuffer {
    fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        use core2::io::Write;
        self.buf.write_all(buf).unwrap();
        Ok(buf.len())
    }
    fn flush(&mut self) -> Result<(), Self::Error> {
        use core2::io::Write;
        self.buf.flush().unwrap();
        Ok(())
    }
}

impl Read for MyBuffer {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        use core2::io::Read;
        self.buf.read_exact(buf).unwrap();
        Ok(buf.len())
    }
}

impl Seek for MyBuffer {
    fn seek(&mut self, pos: SeekFrom) -> Result<u64, Self::Error> {
        use core2::io::Seek;
        let ans = match pos {
            SeekFrom::Start(pos) => self.buf.seek(core2::io::SeekFrom::Start(pos)).unwrap(),
            SeekFrom::End(pos) => self.buf.seek(core2::io::SeekFrom::End(pos)).unwrap(),
            SeekFrom::Current(pos) => self.buf.seek(core2::io::SeekFrom::Current(pos)).unwrap(),
        };
        Ok(ans)
    }
}

impl MyBuffer {
    pub fn new(fat_device: FatDevice) -> Self {
        Self {
            buf: BufStream::new(fat_device),
        }
    }
}

pub const FATFS_SB_OPS: SuperBlockOps = {
    let mut sb_ops = SuperBlockOps::empty();
    sb_ops.stat_fs = simple_statfs;
    sb_ops.sync_fs = fat_sync_fs;
    sb_ops
};

pub const FAT: FileSystemType = {
    FileSystemType::new(
        "fat",
        FileSystemAttr::empty(),
        fat_get_super_blk,
        fat_kill_super_blk,
    )
};

fn fat_get_super_blk(
    fs_type: Arc<FileSystemType>,
    flags: MountFlags,
    dev_name: &str,
    data: Option<Box<dyn DataOps>>,
) -> StrResult<Arc<SuperBlock>> {
    ddebug!("fat get super block");
    assert!(data.is_some());
    let device = data.as_ref().unwrap().as_ref().device(dev_name);
    assert!(device.is_some());
    let device = device.unwrap();
    let fat_device = FatDevice::new(device.clone());
    let fs = fatfs::FileSystem::new(MyBuffer::new(fat_device), fatfs::FsOptions::new()).unwrap();
    let stats = fs.stats();
    if stats.is_err() {
        return Err("read fat data error");
    }
    let stats = stats.unwrap();
    let sb_blk = SuperBlock {
        dev_desc: fs.volume_id(),
        device: Some(device),
        block_size: stats.cluster_size(),
        dirty_flag: false,
        file_max_bytes: usize::MAX,
        mount_flag: flags,
        magic: 0,
        file_system_type: Arc::downgrade(&fs_type),
        super_block_ops: FATFS_SB_OPS,
        blk_dev_name: dev_name.to_string(),
        data,
        inner: Mutex::new(SuperBlockInner::empty()),
    };
    // set the root dentry for super block
    let sb_blk = Arc::new(sb_blk);
    let inode = fat_root_inode(sb_blk.clone(), fs.root_dir());
    let dentry = DirEntry::new(DirFlags::empty(), inode, FAT_DENTRY_OPS, Weak::new(), "/");
    sb_blk.update_root(Arc::new(dentry));
    // inert the super block into file system type
    fs_type.insert_super_blk(sb_blk.clone());
    Ok(sb_blk)
}

fn fat_kill_super_blk(super_blk: Arc<SuperBlock>) {
    let ops = super_blk.super_block_ops.sync_fs;
    ops(super_blk);
}

fn fat_sync_fs(sb_blk: Arc<SuperBlock>) -> StrResult<()> {
    let device = sb_blk.device.as_ref().unwrap().clone();
    let fat_device = FatDevice::new(device);
    let fs = fatfs::FileSystem::new(MyBuffer::new(fat_device), fatfs::FsOptions::new()).unwrap();
    let res = fs.unmount();
    if res.is_err() {
        return Err("sync error");
    }
    Ok(())
}

/// create the root inode for fat file system
fn fat_root_inode(
    sb_blk: Arc<SuperBlock>,
    dir: FatDir,
) -> Arc<Inode> {
    let device = sb_blk.device.as_ref().unwrap().clone();
    let inode = Inode::new(
        sb_blk,
        0,
        0,
        FAT_INODE_DIR_OPS,
        FAT_DIR_FILE_OPS,
        None,
        InodeMode::S_DIR,
    );
    let parent = Arc::new(Mutex::new(dir));
    let fat_inode = FatInode::new(parent.clone(), FatInodeType::Dir(parent));
    inode.access_inner().data = Some(Box::new(fat_inode));
    Arc::new(inode)
}
