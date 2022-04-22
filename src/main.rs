use dedup::server::DeDupServer;

use anyhow::Result;
use tokio::net::TcpListener;
use tokio::runtime::Builder;

fn main() -> Result<()> {
    let rt = Builder::new_multi_thread().enable_all().build()?;
    rt.block_on(async {
        let listener = TcpListener::bind("127.0.0.1:4000").await.unwrap();
        let server = DeDupServer::new(listener).unwrap();
        server.run().await.unwrap();
    });
    Ok(())
}
