use fat32_vfs::fstype::FAT;
use rvfs::file::{vfs_mkdir, vfs_open_file, FileMode, OpenFlags, vfs_write_file, vfs_readdir};
use rvfs::mount::{do_mount, MountFlags};
use rvfs::superblock::{register_filesystem, DataOps, Device};
use rvfs::{init_process_info, mount_rootfs, FakeFSC, PROCESS_FS_CONTEXT};
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::os::unix::fs::FileExt;
use std::sync::Arc;

use rvfs::dentry::Dirent64Iterator;
use rvfs::info::VfsError;
use rvfs::link::vfs_unlink;
use spin::Mutex;

fn main() {
    env_logger::init();
    println!("init vfs");
    let rootfs = mount_rootfs();
    init_process_info(rootfs);
    let file = vfs_open_file::<FakeFSC>("/", OpenFlags::O_RDWR, FileMode::FMODE_WRITE).unwrap();
    println!("file: {:#?}", file);
    register_filesystem(FAT).unwrap();

    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(false)
        .open("fat32.img")
        .unwrap();
    let img = FatImg::new(file);
    let data = Fat32Data::new(Arc::new(img));
    let data = Box::new(data);

    let mnt = do_mount::<FakeFSC>("fake", "/", "fat", MountFlags::empty(), Some(data)).unwrap();
    println!("mnt: {:#?}", mnt);
    PROCESS_FS_CONTEXT.lock().cmnt = mnt.clone();
    PROCESS_FS_CONTEXT.lock().rmnt = mnt.clone();
    PROCESS_FS_CONTEXT.lock().cwd = mnt.root.clone();
    PROCESS_FS_CONTEXT.lock().root = mnt.root.clone();
    let res = vfs_mkdir::<FakeFSC>("/tmp", FileMode::FMODE_WRITE);
    if res.is_err() {
        println!("it has been created");
    }
    let root = vfs_open_file::<FakeFSC>("/", OpenFlags::O_RDWR, FileMode::FMODE_WRITE).unwrap();
    // println!("mnt: {:#?}", mnt);
    let file = vfs_open_file::<FakeFSC>("/test.txt", OpenFlags::O_RDWR|OpenFlags::O_CREAT, FileMode::FMODE_RDWR).unwrap();
    let data = [1u8;1024];
    let mut offset = 0;
    for _i in 0..49*1024{
        vfs_write_file::<FakeFSC>(file.clone(), &data, offset).unwrap();
        offset += 1024;
    }
    println!("offset: {}", offset);
    vfs_unlink::<FakeFSC>("test.txt").unwrap();
    let file = vfs_open_file::<FakeFSC>("/test1.txt", OpenFlags::O_RDWR|OpenFlags::O_CREAT, FileMode::FMODE_RDWR).unwrap();
    readdir(root);
    let data = [1u8;1024];
    let mut offset = 0;
    for _i in 0..40*1024{
        vfs_write_file::<FakeFSC>(file.clone(), &data, offset).expect(
            format!("write error: {}", offset).as_str()
        );
        offset += 4096;
    }
    println!("offset: {}", offset);
}


fn readdir(dir: Arc<rvfs::file::File>) {
    let len = vfs_readdir(dir.clone(), &mut [0; 0]).unwrap();
    assert!(len > 0);
    let mut dirents = vec![0u8; len];

    let r = vfs_readdir(dir, &mut dirents[..]).unwrap();
    assert_eq!(r, len);
    Dirent64Iterator::new(&dirents[..]).for_each(|x| {
        println!("{} {:?} {}",x.get_name(),x.type_,x.ino);
    });
}




#[derive(Debug)]
struct FatImg(Mutex<File>);
impl FatImg {
    pub fn new(file: File) -> Self {
        FatImg(Mutex::new(file))
    }
}

impl Device for FatImg {
    fn read(&self, buf: &mut [u8], offset: usize) -> Result<usize, VfsError> {
        let res = self.0.lock().read_at(buf, offset as u64).unwrap();
        Ok(res)
    }

    fn write(&self, buf: &[u8], offset: usize) -> Result<usize, VfsError> {
        let res = self.0.lock().write_at(buf, offset as u64).unwrap();
        self.0.lock().flush().unwrap();
        Ok(res)
    }

    fn size(&self) -> usize {
        self.0.lock().metadata().unwrap().len() as usize
    }

    fn flush(&self) {
        self.0.lock().flush().unwrap();
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
}
