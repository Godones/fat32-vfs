use fat32_vfs::fstype::FAT;
use rvfs::dentry::{vfs_rename, vfs_truncate};
use rvfs::file::{
    vfs_mkdir, vfs_open_file, vfs_read_file, vfs_readdir, vfs_write_file, OpenFlags, FileMode,
};
use rvfs::mount::{do_mount, MountFlags};
use rvfs::stat::{StatFlags, vfs_getattr};
use rvfs::superblock::{register_filesystem, DataOps, Device};
use rvfs::{init_process_info, mount_rootfs, FakeFSC};
use std::fmt::Debug;
use std::fs::{File, OpenOptions};
use std::os::unix::fs::FileExt;
use std::ptr::null;
use std::sync::Arc;

fn main() {
    env_logger::init();
    let mnt = mount_rootfs();
    init_process_info(mnt);
    register_filesystem(FAT).unwrap();
    vfs_mkdir::<FakeFSC>("/fs", FileMode::FMODE_WRITE).unwrap();
    vfs_mkdir::<FakeFSC>("/fs/fat32", FileMode::FMODE_WRITE).unwrap();
    let file = vfs_open_file::<FakeFSC>("/fs/", OpenFlags::O_RDWR, FileMode::FMODE_WRITE).unwrap();
    vfs_readdir(file).unwrap().into_iter().for_each(|name| {
        println!("name: {}", name);
    });
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(false)
        .open("fat32.img")
        .unwrap();
    let img = FatImg::new(file);
    let data = Fat32Data::new(Arc::new(img));
    let data = Box::new(data);
    let _mount =
        do_mount::<FakeFSC>("fat32", "/fs/fat32", "fat", MountFlags::empty(), Some(data)).unwrap();
    // println!("mount: {:#?}",mount);
    let res = vfs_mkdir::<FakeFSC>("/fs/fat32/tmp", FileMode::FMODE_WRITE);
    if res.is_err() {
        println!("mkdir error, it has been created");
    }
    let file = vfs_open_file::<FakeFSC>(
        "/fs/fat32/hello.txt",
        OpenFlags::O_RDWR | OpenFlags::O_CREAT,
        FileMode::FMODE_WRITE,
    )
    .unwrap();
    let dir =
        vfs_open_file::<FakeFSC>("/fs/fat32/", OpenFlags::O_RDWR, FileMode::FMODE_WRITE).unwrap();
    println!("file: {:#?}", dir);
    vfs_readdir(dir).unwrap().into_iter().for_each(|name| {
        println!("name: {}", name);
    });
    vfs_write_file::<FakeFSC>(file.clone(), "hello world".as_bytes(), 0).unwrap();
    let mut buf = [0u8; 20];
    let len = vfs_read_file::<FakeFSC>(file.clone(), &mut buf, 0).unwrap();
    println!("read: {}", String::from_utf8_lossy(&buf[..len]));

    vfs_truncate::<FakeFSC>("/fs/fat32/hello.txt", 5).unwrap();
    let mut buf = [0u8; 20];
    let len = vfs_read_file::<FakeFSC>(file.clone(), &mut buf, 0).unwrap();
    println!("read: {}", String::from_utf8_lossy(&buf[..len]));
    vfs_rename::<FakeFSC>("/fs/fat32/hello.txt", "/fs/fat32/hello2.txt").unwrap();
    println!("file: {:#?}", file);

    let attr = vfs_getattr::<FakeFSC>("fs/fat32/hello2.txt",StatFlags::empty()).unwrap();
    println!("attr: {:#?}", attr);
    // let attr = vfs_getattr::<FakeFSC>("fs/fat32/u1.txt").unwrap();
    // println!("attr: {:#?}", attr);
}

#[derive(Debug)]
struct FatImg(File);

impl FatImg {
    pub fn new(file: File) -> Self {
        FatImg(file)
    }
}

impl Device for FatImg {
    fn read(&self, buf: &mut [u8], offset: usize) -> Result<usize, ()> {
        let res = self.0.read_at(buf, offset as u64).unwrap();
        Ok(res)
    }

    fn write(&self, buf: &[u8], offset: usize) -> Result<usize, ()> {
        let res = self.0.write_at(buf, offset as u64).unwrap();
        Ok(res)
    }

    fn size(&self) -> usize {
        self.0.metadata().unwrap().len() as usize
    }

    fn flush(&self) {
        self.0.sync_all().unwrap();
    }
}

#[derive(Debug)]
pub struct Fat32Data {
    device: Arc<dyn Device>,
}
impl Fat32Data {
    pub fn new(device: Arc<dyn Device>) -> Self {
        Fat32Data { device }
    }
}

impl DataOps for Fat32Data {
    fn device(&self, _: &str) -> Option<Arc<dyn Device>> {
        Some(self.device.clone())
    }

    fn data(&self) -> *const u8 {
        null()
    }
}
