use anyhow::Result;
use std::io::{Write, IoSlice};
use std::sync::Arc;
use std::thread::sleep;
use std::time::Duration;
use std::{collections::HashSet, sync::atomic::AtomicUsize};
use tokio::sync::Mutex;
use lockfree::{set::Set, stack::Stack};
use tokio::{fs::File, io::AsyncWriteExt};

const CACHE_SIZE: usize = 50000;
#[derive(Debug)]
pub(crate) struct Storage {
    //file_handle: Mutex<File>,
    uniques: Set<u32>,
    cache: Vec<u8>,
    last_stats: Stats,
    new_cache: Arc<Stack<u32>>,
}

#[derive(Debug)]
pub(crate) struct Stats {
    period_uniques: AtomicUsize,
    period_dups: AtomicUsize,
    all_uniques: AtomicUsize,
}

impl Stats {
    pub(crate) fn new() -> Self {
        Self {
            all_uniques: AtomicUsize::new(0),
            period_uniques: AtomicUsize::new(0),
            period_dups: AtomicUsize::new(0),
        }
    }

    pub(crate) fn inc_uniques(&self) {
        self.period_uniques
            .fetch_add(1, std::sync::atomic::Ordering::AcqRel);
    }

    pub(crate) fn inc_dups(&self) {
        self.period_dups.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
    }

    pub(crate) fn print(&self) -> (usize, usize, usize) {
        let u = self.period_uniques.swap(0, std::sync::atomic::Ordering::SeqCst);
        let d = self.period_dups.swap(0, std::sync::atomic::Ordering::SeqCst);
        let prev_len = self.all_uniques.fetch_add(u, std::sync::atomic::Ordering::AcqRel);
        (u, d, prev_len+u)
    }
}

impl Storage {
    pub fn new() -> Result<Self> {
        let std_file = std::fs::File::create("./numbers.log")?;
        //let file_handle = Mutex::new(File::from_std(std_file));
        let uniques = Set::<u32>::new();
        let last_stats = Stats::new();
        let new_cache = Arc::new(Stack::<u32>::new());
        let cl_new_cache = Arc::clone(&new_cache);
        tokio::task::spawn_blocking(move || {
            let ne = Arc::clone(&cl_new_cache);
            let mut std_file = std::fs::File::create("./numbers.log").unwrap();
            loop {
                sleep(Duration::from_millis(1000));
                let mut it = ne.pop_iter();
                let mut big_buf = Vec::<u8>::with_capacity(600000);
                for _ in 0..CACHE_SIZE {
                    if let Some(num) = it.next() {
                        let line = format!("{:0>9}\n", num.to_string());
                        big_buf.extend_from_slice(line.as_bytes());
                    };
                };
                if let Err(e) = std_file.write_vectored(&[IoSlice::new(&big_buf)]){
                    println!("Error during vectored write: {e}");
                };
            }
        });

        Ok(Self {
            //file_handle,
            uniques,
            cache: Vec::with_capacity(CACHE_SIZE),
            last_stats,
            new_cache,
        })
    }
    

    pub async fn append(&self, number: u32) -> Result<()> {
        if !self.check_dup(number) {
            //let line = format!("{:0>9}\n", &number.to_string());
            //let buf = line.as_bytes();
            //self.cache.append(&mut buf.to_vec());
            //if self.cache.len() >= CACHE_SIZE {
            //    self.cache_flush(self.cache.len()).await
            //}
        };
        Ok(())
    }

    //async fn cache_flush(&self, actual_size: usize) {
    //    let mut flushed = false;
    //    let mut written: usize = 0;
    //    let mut buf = self.cache.as_slice();
    //    let mut log_file = self.file_handle.lock().await;
    //    while !flushed {
    //        match log_file.write_buf(&mut buf).await {
    //            Ok(n) if n == 0 => panic!("file is unavailable (may be deleted)"),
    //            Ok(n) if n > 0 && written + n < actual_size => {
    //                written += n;
    //                continue; //previous write wasn't full, just call it again
    //            }
    //            Ok(_) => {
    //                if let Err(e) = log_file.flush().await {
    //                    println!("[-] error during file flus: {e}");
    //                };
    //                flushed = true
    //            }
    //            Err(e) => println!("[-] AsyncIO write error: {e}"),
    //        }
    //    }
    //    self.cache.clear();
    //}

    fn check_dup(&self, number: u32) -> bool {
        if let Err(_e) = self.uniques.insert(number) {
            self.last_stats.inc_dups();
            true
        } else {
            self.last_stats.inc_uniques();
            self.new_cache.push(number);
            false
        }
    }

    pub async fn print_stats(&self) {
        let (new_uniques, dups, all_uniques) = self.last_stats.print();
        println!(
            "Received {new_uniques} unique numbers, {dups} duplicates. Unique total: {all_uniques}"
        );
    }

    pub async fn shutdown(&self) {
        //self.cache_flush(self.cache.len()).await;
    }
}
