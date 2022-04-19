pub mod storage;
pub mod server;

use anyhow::Result;
use tokio::runtime::Runtime;
use server::run_server;
fn main() -> Result<()> {
    let rt = Runtime::new().unwrap();
    Ok(rt.block_on(async {
        run_server().await.unwrap()
    }))
}