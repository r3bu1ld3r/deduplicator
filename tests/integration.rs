extern crate nuclia_lib;
use nuclia_lib::server::{run_server};
use tokio::runtime::Runtime;

fn main() {
    let rt = Runtime::new().unwrap();
    tokio::spawn(async {
        run_server().await
    });

    
}