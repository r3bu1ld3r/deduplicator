extern crate dedup;
use dedup::server::{DeDupServer, InputString};
use rand::{distributions::Alphanumeric, Rng};
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio::time::{sleep, Duration};

struct DedupClient {
    stream: TcpStream,
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
        Self { stream }
    }

    pub async fn send(&mut self, req_type: InputString) {
        let buf = match req_type {
            InputString::ValidNumber(number) => format!("{:0>9}\n", number.to_string()),
            InputString::Termination => format!("terminate\n"),
            InputString::Garbage => generate_trash(),
        };
        self.stream.write_all(buf.as_bytes()).await.unwrap();
    }

    pub async fn _shutdown(&mut self) {
        self.stream.shutdown().await.unwrap();
    }
}

async fn setup_srv() {
    tokio::spawn(async {
        let listener = TcpListener::bind("127.0.0.1:4000").await.unwrap();
        let server = DeDupServer::new(listener).unwrap();
        server.run().await.unwrap();
    });
    sleep(Duration::from_millis(300)).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 6)]
async fn simple_valid_input_test() {
    setup_srv().await;
    let mut client = DedupClient::new().await;
    for n in 1..=10000000 {
        client.send(InputString::ValidNumber(n)).await;
    }
    client.send(InputString::Termination).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn termination_test() {
    setup_srv().await;
    let mut client = DedupClient::new().await;
    client.send(InputString::ValidNumber(18)).await;
    client.send(InputString::ValidNumber(8)).await;
    client.send(InputString::Termination).await;
    client.send(InputString::ValidNumber(28)).await; //TODO assert error here
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn garbage_test() {
    setup_srv().await;
    let mut client = DedupClient::new().await;
    client.send(InputString::ValidNumber(18)).await;
    client.send(InputString::ValidNumber(8)).await;
    client.send(InputString::Garbage).await;
    client.send(InputString::ValidNumber(28)).await; //TODO assert error here
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn clients_limit_test() {
    setup_srv().await;
    for i in 3..=6 {
        let mut client = DedupClient::new().await;
        tokio::spawn(async move {
            let start = i * 10u32.pow(7);
            for n in start..=start + 1 * 20u32.pow(7) {
                client.send(InputString::ValidNumber(n)).await;
            }
            client.send(InputString::Termination).await;
        });
    }
    sleep(Duration::from_secs(60)).await;
}
