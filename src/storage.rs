use anyhow::Result;
use std::{collections::HashSet, sync::atomic::AtomicUsize, time::Duration};

use tokio::{fs::File, io::AsyncWriteExt};

const CACHE_SIZE: usize = 50000;
#[derive(Debug)]
pub(crate) struct Storage {
    file_handle: File,
    uniques: HashSet<u32>,
    cache: Vec<u8>,
    last_stats: Stats,
}

#[derive(Debug)]
pub(crate) struct Stats {
    uniques: AtomicUsize,
    dups: AtomicUsize,
}

impl Stats{
    pub(crate) fn new() -> Self {
        Self {
            uniques: AtomicUsize::new(0),
            dups: AtomicUsize::new(0),
        }
    }

    pub(crate) fn inc_uniques(&self) {
        self.uniques.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
    }

    pub(crate) fn inc_dups(&self) {
        self.dups.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
    }

    pub(crate) fn print(&self) -> (usize, usize) {
        let u = self.uniques.swap(0, std::sync::atomic::Ordering::SeqCst);
        let d = self.dups.swap(0, std::sync::atomic::Ordering::SeqCst);
        (u, d)
    }
}

impl Storage {
    pub async fn new() -> Result<Self> {
        let file_handle = File::create("./numbers.log").await?;
        let uniques = HashSet::<u32>::new();
        let last_stats = Stats::new();
        Ok(Self { file_handle, uniques, cache: Vec::with_capacity(CACHE_SIZE), last_stats })
    }

    pub async fn append(&mut self, number: u32) -> Result<()> {
        if !self.is_dup(number) {
            let line = format!("{}\n", &number.to_string());
            let buf = line.as_bytes();
            self.cache.append(&mut buf.to_vec());
            if self.cache.len() >= CACHE_SIZE {
               self.cache_flush().await
            }
        };
        Ok(())
    }

    async fn cache_flush(&mut self) {
        let mut flushed = false;
        let mut written: usize = 0;
        let target_size = self.cache.len();
        let mut buf = self.cache.as_slice();
        while !flushed {
            match self.file_handle.write_buf(&mut buf).await {
                Ok(n) if n == 0 => panic!("file is unavailable (may be deleted)"),
                Ok(n) if n > 0 && written + n < target_size => {
                    written += n;
                    continue //previous write wasn't full, just call it again 
                },
                Ok(_) => flushed = true,
                Err(e) => println!("[-] AsyncIO write error: {e}"),
            }
        }
        self.cache.clear();
    }

    fn is_dup(&mut self, number: u32) -> bool {
        if self.uniques.insert(number){
            self.last_stats.inc_uniques();
            false
        } else {
            self.last_stats.inc_dups();
            true
        }
    }

    pub async fn print_stats(&self) {
        let (new_uniques, dups) = self.last_stats.print();
        let all_uniques = self.uniques.len();
        println!("Received {new_uniques} unique numbers, {dups} duplicates. Unique total: {all_uniques}");
    }
}
