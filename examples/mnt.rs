use fat32_vfs::fstype::FAT;
use rvfs::file::{vfs_mkdir, vfs_open_file, FileMode, OpenFlags};
use rvfs::mount::{do_mount, MountFlags};
use rvfs::superblock::{register_filesystem, DataOps, Device};
use rvfs::{init_process_info, mount_rootfs, FakeFSC, PROCESS_FS_CONTEXT};
use std::fs::{File, OpenOptions};
use std::os::unix::fs::FileExt;
use std::sync::Arc;

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
    println!("mnt: {:#?}", mnt);
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
}
