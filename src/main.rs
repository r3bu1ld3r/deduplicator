use dedup::server::DeDupServer;

use anyhow::Result;
use tokio::net::TcpListener;
use tokio::runtime::Builder;
use env_logger;

#[cfg(not(target_env = "msvc"))]
use tikv_jemallocator::Jemalloc;

#[cfg(not(target_env = "msvc"))]
#[global_allocator]
static GLOBAL: Jemalloc = Jemalloc;

fn main() -> Result<()> {
    console_subscriber::init();
    env_logger::init();
    let rt = Builder::new_multi_thread().enable_all().build()?;
    rt.block_on(async {
        let listener = TcpListener::bind("127.0.0.1:4000").await.unwrap();
        let server = DeDupServer::new(listener).unwrap();
        server.run().await.unwrap();
    });
    Ok(())
}
