use fatfs::{IoBase, Read, Seek, SeekFrom, Write};
use fscommon::BufStream;
use std::fs::{File, OpenOptions};

struct MyBuffer {
    buf: BufStream<File>,
}

impl IoBase for MyBuffer {
    type Error = ();
}

impl Write for MyBuffer {
    fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        use std::io::Write;
        self.buf.write(buf).unwrap();
        Ok(buf.len())
    }
    fn flush(&mut self) -> Result<(), Self::Error> {
        use std::io::Write;
        self.buf.flush().unwrap();
        Ok(())
    }
}

impl Read for MyBuffer {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        use std::io::Read;
        self.buf.read(buf).unwrap();
        Ok(buf.len())
    }
}

impl Seek for MyBuffer {
    fn seek(&mut self, pos: SeekFrom) -> Result<u64, Self::Error> {
        use std::io::Seek;
        let ans = match pos {
            SeekFrom::Start(pos) => self.buf.seek(core2::io::SeekFrom::Start(pos)).unwrap(),
            SeekFrom::End(pos) => self.buf.seek(core2::io::SeekFrom::End(pos)).unwrap(),
            SeekFrom::Current(pos) => self.buf.seek(core2::io::SeekFrom::Current(pos)).unwrap(),
        };
        Ok(ans)
    }
}

fn main() {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(false)
        .open("fat32.img")
        .unwrap();
    // file.set_len(64 * 1024 * 1024).unwrap();
    let buf_file = BufStream::new(file);
    let mut mybuffer = MyBuffer { buf: buf_file };
    // format_volume(&mut mybuffer, FormatVolumeOptions::new()).unwrap();
    let fs = fatfs::FileSystem::new(mybuffer, fatfs::FsOptions::new()).unwrap();
    let root_dir = fs.root_dir();
    let mut file = root_dir.create_file("root.txt").unwrap();
    println!("----------------------------------");
    file.write_all(b"Hello World!").unwrap();
    let mut buf = [0u8; 100];
    file.seek(SeekFrom::Start(0)).unwrap();
    let len = file.read(&mut buf).unwrap();
    println!("Read {} bytes: {:?}", len, &buf[..len]);
    let f1 = root_dir.create_dir("/d1").unwrap();
    let _file = root_dir.create_dir("/d1/hello.txt").unwrap();
    f1.iter()
        .for_each(|x| println!("{:#?}", x.unwrap().file_name()));
    println!("----------");
    root_dir
        .iter()
        .for_each(|x| println!("{:#?}", x.unwrap().file_name()));
    println!("----------");
    root_dir.rename("root.txt", &f1, "root2.txt").unwrap();
    root_dir
        .iter()
        .for_each(|x| println!("{:#?}", x.unwrap().file_name()));
    println!("----------");
    f1.iter()
        .for_each(|x| println!("{:#?}", x.unwrap().file_name()));
}
