use anyhow::Result;
use std::collections::HashSet;

use tokio::{fs::File, io::AsyncWriteExt};

#[derive(Debug)]
pub struct Storage {
    file_handle: File,
    cache: HashSet<u32>,
}

impl Storage {
    pub async fn new() -> Result<Self> {
        let file_handle = File::create("./numbers.log").await?;
        let cache = HashSet::<u32>::new();
        Ok(Self { file_handle, cache })
    }

    pub async fn append(&mut self, number: u32) -> Result<()> {
        if !self.is_dup(number) {
            let line = format!("{}\n", &number.to_string());
            let mut buf = line.as_bytes();
            self.file_handle.write_buf(&mut buf).await?;
            Ok(())
        } else {
            Ok(())
        }
    }
    
    fn is_dup(&mut self, number: u32) -> bool {
        !self.cache.insert(number)
    }
}
