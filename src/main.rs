use dedup::server::run;

use anyhow::Result;
use tokio::runtime::Runtime;

fn main() -> Result<()> {
    let rt = Runtime::new().unwrap();
    rt.block_on(async { run().await.unwrap() });
    Ok(())
}
