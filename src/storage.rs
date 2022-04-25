use anyhow::Result;
use lockfree::{set::Set, stack::Stack};
use log::debug;
use std::io::{IoSlice, Write};
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::{AtomicBool, Ordering::SeqCst};
use std::sync::Arc;
use std::thread::sleep;
use std::time::Duration;

const CACHE_SIZE: usize = 50000;
#[derive(Debug)]
pub(crate) struct Storage {
    uniques: Set<u32>,
    last_stats: Stats,
    cache: Arc<Stack<u32>>,
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
        self.period_dups
            .fetch_add(1, std::sync::atomic::Ordering::AcqRel);
    }

    pub(crate) fn print(&self) -> (usize, usize, usize) {
        let u = self
            .period_uniques
            .swap(0, std::sync::atomic::Ordering::SeqCst);
        let d = self
            .period_dups
            .swap(0, std::sync::atomic::Ordering::SeqCst);
        let prev_len = self
            .all_uniques
            .fetch_add(u, std::sync::atomic::Ordering::AcqRel);
        (u, d, prev_len + u)
    }
}

impl Storage {
    pub fn new(shutdown: Arc<AtomicBool>) -> Result<Self> {
        let uniques = Set::<u32>::new();
        let last_stats = Stats::new();
        let cache = Arc::new(Stack::<u32>::new());

        Storage::run_writer(Arc::clone(&cache), shutdown);

        Ok(Self {
            uniques,
            last_stats,
            cache,
        })
    }

    fn run_writer(cache: Arc<Stack<u32>>, shutdown: Arc<AtomicBool>) {
        tokio::task::spawn_blocking(move || {
            let ne = Arc::clone(&cache);
            let mut std_file = std::fs::File::create("./numbers.log").unwrap();
            let mut big_buf = Vec::<u8>::with_capacity(CACHE_SIZE * 10 + 20);

            while !shutdown.load(SeqCst) {
                sleep(Duration::from_millis(50));
                let mut it = ne.pop_iter();
                for _ in 0..CACHE_SIZE {
                    if let Some(num) = it.next() {
                        let line = format!("{:0>9}\n", num.to_string());
                        big_buf.extend_from_slice(line.as_bytes());
                    };
                }
                if let Err(e) = std_file.write_vectored(&[IoSlice::new(&big_buf)]) {
                    debug!("Error during vectored write: {e}");
                };
                big_buf.clear();
            }

            while let Some(num) = cache.pop() {
                let line = format!("{:0>9}\n", num.to_string());
                big_buf.extend_from_slice(line.as_bytes());
            }
            if let Err(e) = std_file.write_vectored(&[IoSlice::new(&big_buf)]) {
                debug!("Error during vectored write: {e}, try again");
                let _ = std_file.write_vectored(&[IoSlice::new(&big_buf)]);
            };
        });
    }

    pub async fn append(&self, number: u32) {
        self.inner_append(number);
    }

    fn inner_append(&self, number: u32) {
        if let Err(_e) = self.uniques.insert(number) {
            self.last_stats.inc_dups();
        } else {
            self.last_stats.inc_uniques();
            self.cache.push(number);
        }
    }

    pub async fn print_stats(&self) {
        let (new_uniques, dups, all_uniques) = self.last_stats.print();
        println!(
            "Received {new_uniques} unique numbers, {dups} duplicates. Unique total: {all_uniques}"
        );
    }
}
