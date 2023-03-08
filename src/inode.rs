use crate::file::{FAT_DIR_FILE_OPS, FAT_FILE_FILE_OPS};
use crate::{get_fat_data, FatDir, FatInode, FatInodeType};
use alloc::boxed::Box;
use alloc::sync::Arc;
use fatfs::{Error, Seek};
use rvfs::dentry::DirEntry;
use rvfs::file::{FileMode, FileOps};
use rvfs::inode::{Inode, InodeMode, InodeOps};
use rvfs::superblock::SuperBlock;
use rvfs::{iinfo, info, StrResult};
use spin::Mutex;

pub const FAT_INODE_DIR_OPS: InodeOps = {
    let mut ops = InodeOps::empty();
    ops.create = fat_create;
    ops.mkdir = fat_mkdir;
    ops.rmdir = fat_rmdir;
    ops.rename = fat_rename;
    ops
};

pub const FAT_INODE_FILE_OPS: InodeOps = {
    let mut ops = InodeOps::empty();
    ops.truncate = fat_truncate;
    ops
};

fn fat_truncate(inode: Arc<Inode>) -> StrResult<()> {
    let fat_data = get_fat_data(inode.clone());
    let inode_inner = inode.access_inner();
    let file_size = inode_inner.file_size;
    let parent = &fat_data.parent;
    let parent = parent.lock();
    if let FatInodeType::File(name) = &fat_data.current {
        let file = parent.open_file(name);
        if file.is_err() {
            return Err("Open file failed");
        }
        let mut file = file.unwrap();
        let res = file.seek(fatfs::SeekFrom::Start(file_size as u64));
        if res.is_err() {
            return Err("Seek file failed");
        }
        let res = file.truncate();
        if res.is_err() {
            return Err("Truncate file failed");
        }
    } else {
        return Err("Not a file");
    }
    Ok(())
}

fn fat_mkdir(dir: Arc<Inode>, dentry: Arc<DirEntry>, _mode: FileMode) -> StrResult<()> {
    iinfo!("fat_mkdir");
    let fat_data = get_fat_data(dir.clone());
    let name = dentry.access_inner().d_name.clone();
    let res = __fat_create_dir_or_file(fat_data, true, &name);
    let parent_dir = match res {
        Ok(dir) => dir,
        Err(Error::InvalidInput) => return Err("File exist"),
        Err(Error::NotEnoughSpace) => return Err("No space"),
        Err(Error::Io(_)) => return Err("IO error"),
        _ => return Err("Unknown error"),
    };
    let sb_blk = dir.super_blk.upgrade().unwrap();
    let current = parent_dir.lock().open_dir(&name).unwrap();
    let current = FatInodeType::Dir(Arc::new(Mutex::new(current)));
    // create a inode for the dentry
    let inode = generate_fat_inode(
        sb_blk,
        FAT_INODE_DIR_OPS,
        FAT_DIR_FILE_OPS,
        InodeMode::S_DIR,
        parent_dir,
        current,
    );
    // set the dentry's inode
    dentry.access_inner().d_inode = inode;
    iinfo!("fat_mkdir end");
    Ok(())
}

fn fat_rmdir(dir: Arc<Inode>, dentry: Arc<DirEntry>) -> StrResult<()> {
    let fat_data = get_fat_data(dir);
    let name = dentry.access_inner().d_name.clone();
    let res = __fat_remove_dir_or_file(fat_data, &name);
    match res {
        Ok(_) => {}
        Err(Error::InvalidInput) => return Err("File not exist"),
        Err(Error::NotEnoughSpace) => return Err("No space"),
        Err(Error::Io(_)) => return Err("IO error"),
        _ => return Err("Unknown error"),
    };
    Ok(())
}

fn fat_create(dir: Arc<Inode>, dentry: Arc<DirEntry>, _mode: FileMode) -> StrResult<()> {
    let fat_data = get_fat_data(dir.clone());
    let name = dentry.access_inner().d_name.clone();
    let res = __fat_create_dir_or_file(fat_data, false, &name);
    let parent = match res {
        Ok(dir) => dir,
        Err(Error::NotEnoughSpace) => return Err("No space"),
        Err(Error::Io(_)) => return Err("IO error"),
        _ => return Err("Unknown error"),
    };
    let sb_blk = dir.super_blk.upgrade().unwrap();
    let current = FatInodeType::File(name.clone());
    // create a inode for the dentry
    let inode = generate_fat_inode(
        sb_blk,
        FAT_INODE_FILE_OPS,
        FAT_FILE_FILE_OPS,
        InodeMode::S_FILE,
        parent,
        current,
    );
    // set the dentry's inode
    dentry.access_inner().d_inode = inode;
    Ok(())
}

// TODO! solve it
fn fat_rename(
    dir: Arc<Inode>,
    old_dentry: Arc<DirEntry>,
    new_dir: Arc<Inode>,
    new_dentry: Arc<DirEntry>,
) -> StrResult<()> {
    let old_name = old_dentry.access_inner().d_name.clone();
    let new_name = new_dentry.access_inner().d_name.clone();
    // whether the dir is equal to the new_dir
    let is_same_dir = Arc::ptr_eq(&dir, &new_dir);
    let old_fat_data = get_fat_data(dir);
    if is_same_dir {
        // rename in the same dir
        if let FatInodeType::Dir(dir) = &old_fat_data.current {
            let dir = dir.lock();
            let res = dir.rename(&old_name, &(*dir), &new_name);
            match res {
                Ok(_) => {}
                Err(Error::AlreadyExists) => {
                    // try delete the target src
                    let res = dir.remove(&new_name);
                    if res.is_err() {
                        return Err("fat error");
                    }
                    dir.rename(&old_name, &(*dir), &new_name).unwrap();
                }
                _ => return Err("fat error"),
            }
        } else {
            return Err("It is not a dir");
        }
    } else {
        let new_fat_data = get_fat_data(new_dir);
        if let FatInodeType::Dir(old_dir) = &old_fat_data.current
            && let FatInodeType::Dir(new_dir) = &new_fat_data.current {
            let old_dir = old_dir.lock();
            let new_dir = new_dir.lock();
            let res = old_dir.rename(&old_name, &(*new_dir), &new_name);
            match res {
                Ok(_) => {}
                Err(Error::AlreadyExists) => {
                    // try delete the target src
                    let res = new_dir.remove(&new_name);
                    if res.is_err(){
                        return Err("fat error")
                    }
                    old_dir.rename(&old_name,&(*new_dir),&new_name).unwrap();
                }
                _ => return Err("fat error")
            }
        }else {
            return Err("It is not a dir")
        }
    }
    //todo
    Ok(())
}

/// user should set the file size in the inode after calling this function
fn generate_fat_inode(
    sb_blk: Arc<SuperBlock>,
    inode_ops: InodeOps,
    file_ops: FileOps,
    mode: InodeMode,
    parent: Arc<Mutex<FatDir>>,
    current: FatInodeType,
) -> Arc<Inode> {
    let inode = Inode::new(sb_blk, 0, 0, inode_ops, file_ops, None, mode);
    // add fat data
    let fat_data = FatInode::new(parent, current);
    let fat_data = Box::new(fat_data);
    inode.access_inner().data = Some(fat_data);
    Arc::new(inode)
}
fn __fat_create_dir_or_file(
    fat_data: &mut FatInode,
    is_dir: bool,
    name: &str,
) -> Result<Arc<Mutex<FatDir>>, Error<()>> {
    iinfo!("create dir or file");
    info!("name: {}", name);
    let current = &fat_data.current;
    let dir = match current {
        FatInodeType::Dir(dir) => {
            let dir_lock = dir.lock();
            if is_dir {
                dir_lock.create_dir(name)?;
            } else {
                let mut file = dir_lock.create_file(name)?;
                // make file size to 0
                file.truncate()?;
            };
            drop(dir_lock);
            dir.clone()
        }
        _ => {
            return Err(Error::InvalidInput);
        }
    };
    iinfo!("create dir or file success");
    Ok(dir)
}

fn __fat_remove_dir_or_file(fat_data: &mut FatInode, name: &str) -> Result<(), Error<()>> {
    let current = &fat_data.current;
    match current {
        FatInodeType::Dir(dir) => {
            let dir_lock = dir.lock();
            dir_lock.remove(name)?;
        }
        _ => {
            return Err(Error::InvalidInput);
        }
    };
    Ok(())
}
