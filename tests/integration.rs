extern crate dedup;
use dedup::server::{run, InputString};
use tokio::time::sleep;
use std::io::Read;
use rand::{distributions::Alphanumeric, Rng}; 
use tokio::io::AsyncWriteExt;
use tokio::{net::TcpStream, runtime::Runtime};


struct DedupClient {
    stream: TcpStream
}

fn generate_trash() -> String {
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(9)
        .map(char::from)
        .collect()
}

impl DedupClient {
    pub async fn new() -> Self {
        let stream = TcpStream::connect("127.0.0.1:4000").await.unwrap();
        stream.writable().await.unwrap();
        Self {
            stream
        }
    }

    pub async fn send(&mut self, req_type: InputString) {
        let buf = match req_type {
            InputString::Data(number) => format!("{:0>9}\n", number.to_string()),
            InputString::Termination => format!("terminate\n"),
            InputString::Garbage => generate_trash(),
        };
        self.stream.write_all(buf.as_bytes()).await.unwrap();
    }

    pub async fn shutdown(&mut self) {
        self.stream.shutdown().await.unwrap();
    }
}

async fn setup_srv() {
    tokio::spawn(async {
        run().await.unwrap();
    });
    sleep(std::time::Duration::from_secs(1)).await;
}

#[tokio::test]
async fn simple_valid_input_test() {
    setup_srv().await;
    let mut client = DedupClient::new().await;
    for n in 1000..10000 {
        client.send(InputString::Data(n)).await;
        if n % 13 == 0 {
            client.send(InputString::Data(n)).await
        }
    }
    //TODO add assert number of uniques
}

#[tokio::test]
async fn termination_test () {
    setup_srv().await;
    let mut client = DedupClient::new().await;
    client.send(InputString::Data(18)).await;
    client.send(InputString::Data(8)).await;
    client.send(InputString::Termination).await;
    client.send(InputString::Data(28)).await; //TODO assert error here
}

#[tokio::test]
async fn garbage_test() {
    setup_srv().await;
    let mut client = DedupClient::new().await;
    client.send(InputString::Data(18)).await;
    client.send(InputString::Data(8)).await;
    client.send(InputString::Garbage).await;
    client.send(InputString::Data(28)).await; //TODO assert error here
}

#[tokio::test]
async fn clients_limit_test() {
}