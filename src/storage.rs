use anyhow::Result;
use std::collections::HashSet;

use tokio::fs::File;

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

    pub async fn append(number: u32) -> Result<()> {
        Ok(())
    }
}
