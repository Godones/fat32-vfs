use crate::{get_fat_data, FatInodeType};
use alloc::sync::Arc;
use alloc::vec::Vec;
use fatfs::{Read, Seek, Write};
use rvfs::dentry::{DirContext, DirEntryOps};
use rvfs::file::{File, FileOps};
use rvfs::{StrResult,};

pub const FAT_FILE_FILE_OPS: FileOps = {
    let mut file_ops = FileOps::empty();
    file_ops.read = fat_read_file;
    file_ops.write = fat_write_file;
    file_ops.open = |_| Ok(());
    file_ops
};

pub const FAT_DIR_FILE_OPS: FileOps = {
    let mut dir_ops = FileOps::empty();
    dir_ops.readdir = fat_readdir;
    dir_ops.open = |_| Ok(());
    dir_ops.flush = fat_flush;
    dir_ops.fsync = fat_fsync;
    dir_ops
};

pub const FAT_DENTRY_OPS: DirEntryOps = DirEntryOps::empty();

fn fat_read_file(file: Arc<File>, buf: &mut [u8], offset: u64) -> StrResult<usize> {
    let inode = file.f_dentry.access_inner().d_inode.clone();
    let fat_data = get_fat_data(inode);
    let parent = &fat_data.parent;
    return if let FatInodeType::File(name) = &fat_data.current {
        let file = parent.lock().open_file(name);
        if file.is_err() {
            return Err("Open file failed");
        }
        let mut file = file.unwrap();
        let res = file.seek(fatfs::SeekFrom::Start(offset));
        if res.is_err() {
            return Err("Seek file failed");
        }
        let mut buf = buf;
        let mut count = 0;
        while buf.len() > 0 {
            let res = file.read(buf);
            if res.is_err() {
                return Err("Read file failed");
            }
            let len = res.unwrap();
            if len == 0 {
                break;
            }
            count += len;
            buf = &mut buf[len..];
        };

        Ok(count)
    } else {
        Err("Not a file")
    };
}
fn fat_write_file(file: Arc<File>, buf: &[u8], offset: u64) -> StrResult<usize> {
    let inode = file.f_dentry.access_inner().d_inode.clone();
    let fat_data = get_fat_data(inode);
    let parent = &fat_data.parent;
    return if let FatInodeType::File(name) = &fat_data.current {
        let file = parent.lock().open_file(name);
        if file.is_err() {
            return Err("Open file failed");
        }
        let mut file = file.unwrap();
        let res = file.seek(fatfs::SeekFrom::Start(offset));
        if res.is_err() {
            return Err("Seek file failed");
        }
        let res = file.write_all(buf);
        if res.is_err() {
            return Err("Write file failed");
        }
        Ok(buf.len())
    } else {
        Err("Not a file")
    };
}

fn fat_readdir(file: Arc<File>) -> StrResult<DirContext> {
    let inode = file.f_dentry.access_inner().d_inode.clone();
    let fat_data = get_fat_data(inode);
    return if let FatInodeType::Dir(dir) = &fat_data.current {
        let mut data = Vec::new();
        dir.lock().iter().for_each(|x| {
            if let Ok(x) = x {
                data.extend_from_slice(x.file_name().as_bytes());
                data.push(0);
            }
        });
        Ok(DirContext::new(data))
    } else {
        Err("Not a dir")
    };
}

fn fat_flush(file: Arc<File>) -> StrResult<()> {
    let inode = file.f_dentry.access_inner().d_inode.clone();
    let fat_data = get_fat_data(inode);
    let parent = &fat_data.parent;
    return if let FatInodeType::File(name) = &fat_data.current {
        let file = parent.lock().open_file(name);
        if file.is_err() {
            return Err("Open file failed");
        }
        let mut file = file.unwrap();
        let res = file.flush();
        if res.is_err() {
            return Err("Flush file failed");
        }
        Ok(())
    } else {
        Err("Not a file")
    };
}

fn fat_fsync(file: Arc<File>, _datasync: bool) -> StrResult<()> {
    fat_flush(file)
}
