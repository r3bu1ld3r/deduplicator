extern crate nuclia;
use nuclia::server::run;
use tokio::{runtime::Runtime, net::{TcpSocket, TcpStream}};

fn main() {
    let rt = Runtime::new().unwrap();
    tokio::spawn(async {
        run().await
    });

    rt.block_on(async {
        run_client().await
    });
}

async fn run_client() {
    loop {
        let stream = TcpStream::connect("127.0.0.1:4000");
    }
}