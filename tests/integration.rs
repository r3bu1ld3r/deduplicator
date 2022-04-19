extern crate dedup;
use dedup::server::run;
use std::thread::sleep;
use tokio::io::AsyncWriteExt;
use tokio::{net::TcpStream, runtime::Runtime};

fn setup_srv(rt: &Runtime) {
    rt.spawn(async {
        run().await.unwrap();
        unreachable!("can't executed before server poweroff");
    });
    sleep(std::time::Duration::from_secs(1));
}

#[test]
fn valid_input_test() {
    let rt = Runtime::new().unwrap();
    setup_srv(&rt);
    rt.block_on(async {
        let mut stream = TcpStream::connect("127.0.0.1:4000").await.unwrap();
        stream.write_all(b"012345678\n").await.unwrap();
        stream.shutdown().await.unwrap();
    });
    sleep(std::time::Duration::from_secs(5));
}
