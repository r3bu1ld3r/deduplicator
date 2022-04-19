use nuclia::server::run;

use anyhow::Result;
use tokio::runtime::Runtime;

fn main() -> Result<()> {
    let rt = Runtime::new().unwrap();
    Ok(rt.block_on(async {
        run().await.unwrap()
    }))
}