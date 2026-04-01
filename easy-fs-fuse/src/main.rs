use clap::Parser;
use easy_fs::{BlockDevice, EasyFileSystem};
use std::fs::{read_dir, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::sync::Arc;
use std::sync::Mutex;

const BLOCK_SZ: usize = 512;

struct BlockFile(Mutex<File>);

impl BlockDevice for BlockFile {
    fn read_block(&self, block_id: usize, buf: &mut [u8]) {
        let mut file = self.0.lock().unwrap();
        file.seek(SeekFrom::Start((block_id * BLOCK_SZ) as u64))
            .expect("Error when seeking!");
        assert_eq!(file.read(buf).unwrap(), BLOCK_SZ, "Not a complete block!");
    }

    fn write_block(&self, block_id: usize, buf: &[u8]) {
        let mut file = self.0.lock().unwrap();
        file.seek(SeekFrom::Start((block_id * BLOCK_SZ) as u64))
            .expect("Error when seeking!");
        assert_eq!(file.write(buf).unwrap(), BLOCK_SZ, "Not a complete block!");
    }
}

fn main() {
    easy_fs_pack().expect("Error when packing easy-fs!");
}

#[derive(clap::Parser)]
#[clap(version = "1.0", about = "EasyFileSystem packer")]
struct Cli {
    #[clap(short, long, help = "Executable source dir(with backslash)")]
    source: String,
    #[clap(short, long, help = "Executable target dir(with backslash)")]
    target: String,
}

fn easy_fs_pack() -> std::io::Result<()> {
    let cli = Cli::parse();
    let block_file = Arc::new(BlockFile(Mutex::new({
        let f = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(format!("{}{}", cli.target, "fs.img"))?;
        f.set_len(16 * 2048 * 512).unwrap();
        f
    })));
    // 16MiB, at most 4095 files
    let efs = EasyFileSystem::create(block_file, 16 * 2048, 1);
    let root_inode = Arc::new(EasyFileSystem::root_inode(&efs));
    let test_node = root_inode.create_dir("test").unwrap();

    let entries = read_dir(cli.source)?;
    for dir_entry in entries {
        let path = dir_entry?.path();
        if path.is_file() {
            let mut file_name = path.file_name().unwrap().to_str().unwrap().to_string();
            if let Some(pos) = file_name.find('.') {
                file_name.drain(pos..file_name.len());
            }
            let bin_path = cli.target.clone() + file_name.as_str();
            let mut host_file = File::open(bin_path)?;
            let mut all_data: Vec<u8> = Vec::new();
            host_file.read_to_end(&mut all_data)?;

            let target_inode = if file_name.starts_with("test_") {
                let file_name = file_name.strip_prefix("test_").unwrap().to_string();
                test_node.create(file_name.as_str()).unwrap()
            } else {
                root_inode.create(file_name.as_str()).unwrap()
            };
            target_inode.write_at(0, all_data.as_slice());
        }
    }

    Ok(())
}
