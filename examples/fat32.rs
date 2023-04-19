use fatfs::{IoBase, Read, Seek, SeekFrom, Write};
use std::fs::OpenOptions;
fn main() {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(false)
        .open("fat32.img")
        .unwrap();
    // file.set_len(64 * 1024 * 1024).unwrap();
    let buf_file = BufStream::new(file);
    // format_volume(&mut mybuffer, FormatVolumeOptions::new()).unwrap();
    let fs = fatfs::FileSystem::new(buf_file, fatfs::FsOptions::new()).unwrap();
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

    let mut test = root_dir.create_file("test.txt").unwrap();
    let buf = [0u8; 4090];
    // for i in 0..buf.len() {
    //     buf[i] = rand::random();
    // }
    test.write_all(&buf).unwrap();
    let offset = test.offset();
    println!("Offset: {}", offset);
    test.seek(SeekFrom::Start(0)).unwrap();
    let mut nbuf = [0u8; 512];
    let mut count = 0;
    loop {
        let len = test.read(&mut nbuf).unwrap();
        if len == 0 {
            break;
        }
        assert_eq!(nbuf[..len], buf[count..count + len]);
        count += len;
        println!("Read {} bytes", len);
    }
}

struct BufStream {
    file: std::fs::File,
}

impl BufStream {
    pub fn new(file: std::fs::File) -> Self {
        BufStream { file }
    }
}

impl IoBase for BufStream {
    type Error = ();
}

impl Read for BufStream {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        std::io::Read::read(&mut self.file, buf).map_err(|_| ())
    }
}

impl Write for BufStream {
    fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        std::io::Write::write(&mut self.file, buf).map_err(|_| ())
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        std::io::Write::flush(&mut self.file).map_err(|_| ())
    }
}

impl Seek for BufStream {
    fn seek(&mut self, pos: SeekFrom) -> Result<u64, Self::Error> {
        match pos {
            SeekFrom::Start(pos) => {
                std::io::Seek::seek(&mut self.file, std::io::SeekFrom::Start(pos)).map_err(|_| ())
            }
            SeekFrom::End(pos) => {
                std::io::Seek::seek(&mut self.file, std::io::SeekFrom::End(pos)).map_err(|_| ())
            }
            SeekFrom::Current(pos) => {
                std::io::Seek::seek(&mut self.file, std::io::SeekFrom::Current(pos)).map_err(|_| ())
            }
        }
    }
}
