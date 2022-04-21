use dedup::server::DeDupServer;

use anyhow::Result;
use tokio::runtime::{Builder, Runtime};

fn main() -> Result<()> {
    let rt = Builder::new_multi_thread().unwrap();
    rt.block_on(async { 
        let listener = TcpListener::bind("127.0.0.1:4000").await?;
        let server = DeDupServer::new(listener).unwrap();
        server.run().await.unwrap() 
    });
    Ok(())
}
