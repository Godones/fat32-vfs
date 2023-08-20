use crate::{get_fat_data, FatInodeType};
use alloc::sync::Arc;
use alloc::vec;
use core::cmp::max;

use fatfs::{Read, Seek, SeekFrom, Write};
use log::debug;
use rvfs::dentry::{DirEntryOps, Dirent64, DirentType};
use rvfs::file::{File, FileOps};
use rvfs::StrResult;
pub const FAT_FILE_FILE_OPS: FileOps = {
    let mut file_ops = FileOps::empty();
    file_ops.read = fat_read_file;
    file_ops.write = fat_write_file;
    file_ops.open = |_| Ok(());
    file_ops.llseek = fat_llseek;
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
    debug!("fat read {} {}", buf.len(), offset);
    let inode = file.f_dentry.access_inner().d_inode.clone();
    let fat_data = get_fat_data(inode);
    let _parent = &fat_data.parent;
    return if let FatInodeType::File((_name, file)) = &fat_data.current {
        if file.is_none() {
            return Err("Open file failed");
        }
        let mut file = file.as_ref().unwrap().lock();
        if file.offset() != offset as u32 {
            file.seek(SeekFrom::Start(offset))
                .map_err(|_| "Seek failed")?;
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
        }
        Ok(count)
    } else {
        Err("Not a file")
    };
}
fn fat_write_file(file: Arc<File>, buf: &[u8], offset: u64) -> StrResult<usize> {
    // warn!("fat write {} {}",buf.len(),offset);
    let inode = file.f_dentry.access_inner().d_inode.clone();
    let f_size = file
        .f_dentry
        .access_inner()
        .d_inode
        .access_inner()
        .file_size;
    let fat_data = get_fat_data(inode);
    let _parent = &fat_data.parent;
    return if let FatInodeType::File((_name, file)) = &fat_data.current {
        if file.is_none() {
            return Err("Open file failed");
        }
        let mut file = file.as_ref().unwrap().lock();

        if f_size < offset as usize {
            let max_offset = max(offset as usize, file.offset() as usize);
            if max_offset > f_size {
                let data = vec![0u8; max_offset - f_size];
                file.write_all(&data).map_err(|_| "Write file failed")?;
            }
        }

        if file.offset() != offset as u32 {
            file.seek(SeekFrom::Start(offset))
                .map_err(|_| "Seek file failed")?;
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

fn fat_readdir(file: Arc<File>, dirents: &mut [u8]) -> StrResult<usize> {
    let mut file_inner = file.access_inner();
    let f_pos = file_inner.f_pos;
    let inode = file.f_dentry.access_inner().d_inode.clone();
    let fat_data = get_fat_data(inode);

    let mut read_num = 0;
    return if let FatInodeType::Dir(dir) = &fat_data.current {
        let value = if dirents.is_empty() {
            dir.lock()
                .iter()
                .map(|x| {
                    if let Ok(x) = x {
                        let fake_dirent = Dirent64::new(&x.file_name(), 1, 0, DirentType::empty());
                        fake_dirent.len()
                    } else {
                        0
                    }
                })
                .sum::<usize>()
        } else {
            let mut count = 0;
            let buf_len = dirents.len();
            let mut ptr = dirents.as_mut_ptr();
            dir.lock()
                .iter()
                .skip(f_pos)
                .enumerate()
                .for_each(|(index, x)| {
                    if let Ok(sub_file) = x {
                        let type_ = if sub_file.is_file() {
                            DirentType::DT_REG
                        } else if sub_file.is_dir() {
                            DirentType::DT_DIR
                        } else {
                            DirentType::empty()
                        };
                        let dirent = Dirent64::new(&sub_file.file_name(), 1, index as i64, type_);
                        if count + dirent.len() <= buf_len {
                            let dirent_ptr = unsafe { &mut *(ptr as *mut Dirent64) };
                            *dirent_ptr = dirent;
                            let name_ptr = dirent_ptr.name.as_mut_ptr();
                            unsafe {
                                let mut name = sub_file.file_name().clone();
                                name.push('\0');
                                let len = name.len();
                                name_ptr.copy_from(name.as_ptr(), len);
                                ptr = ptr.add(dirent_ptr.len());
                            }
                            count += dirent_ptr.len();
                            read_num += 1;
                        } else {
                            return;
                        }
                    }
                });
            count
        };
        file_inner.f_pos += read_num;
        Ok(value)
    } else {
        Err("Not a dir")
    };
}

fn fat_flush(file: Arc<File>) -> StrResult<()> {
    let inode = file.f_dentry.access_inner().d_inode.clone();
    let fat_data = get_fat_data(inode);
    let _parent = &fat_data.parent;
    return if let FatInodeType::File((_name, file)) = &fat_data.current {
        if file.is_none() {
            return Err("Open file failed");
        }
        let mut file = file.as_ref().unwrap().lock();
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

fn fat_llseek(_file: Arc<File>, _whence: rvfs::file::SeekFrom) -> StrResult<u64> {
    Err("Not support")
}
