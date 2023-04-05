# fat with VFS

Provide vfs interface for fat file system

## description

fat filesystem from project [rafalh/rust-fatfs: A FAT filesystem library implemented in Rust. (github.com)](https://github.com/rafalh/rust-fatfs)，but it has been modified. In the source code, the `Cell/RefCell` is changed to `Mutex`, `&` changed to `Arc`. And its dependent project `fs-common` has also been moved.


## functions

Due to the limitation of fat32 function, we can only support a part of vfs functions.

```
fn fat_read_file(file: Arc<File>, buf: &mut [u8], offset: u64) -> StrResult<usize>
fn fat_write_file(file: Arc<File>, buf: &[u8], offset: u64) -> StrResult<usize>
fn fat_readdir(file: Arc<File>) -> StrResult<DirContext>
fn fat_flush(file: Arc<File>) -> StrResult<()>
fn fat_fsync(file: Arc<File>, _datasync: bool) -> StrResult<()>
fn fat_truncate(inode: Arc<Inode>) -> StrResult<()>
fn fat_mkdir(dir: Arc<Inode>, dentry: Arc<DirEntry>, _mode: FileMode) -> StrResult<()>
fn fat_rmdir(dir: Arc<Inode>, dentry: Arc<DirEntry>) -> StrResult<()>
fn fat_create(dir: Arc<Inode>, dentry: Arc<DirEntry>, _mode: FileMode) -> StrResult<()>
fn fat_rename(
    dir: Arc<Inode>,
    old_dentry: Arc<DirEntry>,
    new_dir: Arc<Inode>,
    new_dentry: Arc<DirEntry>,
) -> StrResult<()>
fn fat_lookup(p_dir: Arc<Inode>, dentry: Arc<DirEntry>) -> StrResult<()>
```


## Design

For the better performance, we use the  `data` field of the inode to save the file information opened in fat32. If we 
don't do this, every time we open the file, we will start searching from the root directory.

The data structure is 
``` rust
pub struct FatInode {
    // parent
    pub parent: Arc<Mutex<FatDir>>,
    // current: if the file is a directory,then the current is the directory's DIR struct.
    pub current: FatInodeType,
}

pub enum FatInodeType {
    Dir(Arc<Mutex<FatDir>>),
    File(String),
}
```
According to this design, we need to be careful when rename happens, because the parent of the inode may change.



## Usage

The fat filesystem needs a block device (or a file) to read and write data, so we need to implement the 'Device' trait for the block device. 
The 'Device' trait is defined in the `rvfs` project. In this project, we will implement the `Read` `Write` `Seek` and other traits for
the Wrapper of the block device. The Wrapper will be used as the parameter of the `fatfs::FileSystem::new` function. The block device will be 
used as the mount parameter.


``` rust
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



/// define the block device
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
```