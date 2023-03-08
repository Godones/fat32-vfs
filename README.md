# fat with VFS

Provide vfs interface for fat file system

## description

fat32 filesystem from project [rafalh/rust-fatfs: A FAT filesystem library implemented in Rust. (github.com)](https://github.com/rafalh/rust-fatfs)，but it has been modified. In the source code, the `Cell/RefCell` is changed to `Mutex`, `&` changed to `Arc`. And its dependent project `fs-common` has also been modified, and the dependency on `core-io` has been updated to `core2` 



