use anyhow::Result;
use std::collections::HashSet;

use tokio::{fs::File, io::AsyncWriteExt};

#[derive(Debug)]
pub struct Storage {
    file_handle: File,
    uniques: HashSet<u32>,
    cache: Vec<u8>,
}

impl Storage {
    pub async fn new() -> Result<Self> {
        let file_handle = File::create("./numbers.log").await?;
        let uniques = HashSet::<u32>::new();
        Ok(Self { file_handle, uniques, cache: vec![] })
    }

    pub async fn append(&mut self, number: u32) -> Result<()> {
        if !self.is_dup(number) {
            let line = format!("{}\n", &number.to_string());
            let buf = line.as_bytes();
            self.cache.append(&mut buf.to_vec());
            if self.cache.len() >= 5000 {
                if let Err(e) = self.file_handle.write_buf(&mut self.cache.as_slice()).await {
                    println!("AsyncIO error: {e}")
                };
                self.cache.clear();
            }
        };
        Ok(())
    }

    fn is_dup(&mut self, number: u32) -> bool {
        !self.uniques.insert(number)
    }
}
