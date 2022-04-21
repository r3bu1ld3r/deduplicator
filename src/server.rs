use std::{sync::Arc, time::Duration};

use crate::storage::Storage;
use anyhow::{anyhow, Result};
use tokio::{
    io::AsyncWriteExt,
    net::{TcpListener, TcpStream},
    sync::{
        broadcast::{self, Receiver, Sender},
        Mutex, Semaphore, mpsc,
    }, runtime::{Runtime, Builder},
};

//pub struct DeDupServer {
//    listener: TcpListener,
//    conn_limit: Arc<Semaphore>,
//    storage: Arc<Mutex<Storage>>,
//    termination_notify: broadcast::Sender<()>,
//    shutdown_complete_rx: mpsc::Receiver<()>,
//    shutdown_complete_tx: mpsc::Sender<()>,
//}
//
//const MAX_CONNECTIONS: usize = 5;
//
//impl DeDupServer{
//    pub fn new() -> Self {
//        let rt = Builder::new_multi_thread()
//            .enable_all()
//            .build()
//            .unwrap();
//        
//        let conn_limit = Arc::new(Semaphore::new(MAX_CONNECTIONS));
//
//    }
//}

#[derive(Debug, PartialEq)]
pub enum InputString {
    ValidNumber(u32),
    Termination,
    Garbage,
}

pub async fn run() -> Result<()> {
    let listener = TcpListener::bind("127.0.0.1:4000").await?;
    let clients_limit = Arc::new(Semaphore::new(5));
    let storage = Arc::new(Mutex::new(Storage::new().await?));
    let (tx, _) = broadcast::channel::<bool>(5);

    let storage_cln = Arc::clone(&storage);
    tokio::spawn(async move{
        let mut interval = tokio::time::interval(Duration::from_secs(10));
        loop{
            interval.tick().await;
            let store = storage_cln.lock().await;
            store.print_stats().await;
        }
    });

    loop {
        let (socket, _) = listener.accept().await?;
        let limit_cln = Arc::clone(&clients_limit);
        let storage_cln = Arc::clone(&storage);
        let sender = tx.clone();
        let subscriber = tx.subscribe();

        tokio::spawn(async move {
            if let Ok(_guard) = limit_cln.try_acquire() {
                if let Err(e) = client_handler(socket, storage_cln, sender, subscriber).await {
                    println!("[-] Error: {e}")
                }
            };
        });
    }
}

pub(crate) async fn client_handler(
    mut stream: TcpStream,
    storage: Arc<Mutex<Storage>>,
    sender: Sender<bool>,
    mut subscriber: Receiver<bool>,
) -> Result<()> {
    let mut watcher = true;
    while watcher {
        watcher = tokio::select! {
            res = read_number(&mut stream, &storage, &sender) => res?,
            _ = subscriber.recv() => false,
        }
    }
    Ok(())
}

pub(crate) async fn read_number(
    stream: &mut TcpStream,
    storage: &Arc<Mutex<Storage>>,
    sender: &Sender<bool>,
) -> Result<bool> {
    let mut input_buf = [0; 10];
    loop{
        stream.readable().await?;
        match stream.try_read(&mut input_buf){
            Ok(n) if n == 0 => return Ok(false), //stream closed by client
            Ok(_) => break,
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => continue,
            Err(e) => return Err(anyhow!("{e}"))
        }
    }

    return match parse_input(input_buf).await? {
        InputString::ValidNumber(n) => {
            let mut store = storage.lock().await;
            store.append(n).await?;
            Ok(true)
        }
        InputString::Termination => {
            println!("[+] Termination msg received by: {}", sender.send(true)?);
            Ok(false)
        }
        InputString::Garbage => {
            stream.shutdown().await?;
            Ok(false)
        }
    }
}

pub(crate) async fn parse_input(input: [u8; 10]) -> Result<InputString> {
    if input[9] != 0xA {
        Ok(InputString::Garbage)
    } else {
        match std::str::from_utf8(&input[0..9]) {
            Ok(s) if s.eq("terminate") => Ok(InputString::Termination),
            Ok(s) if s.chars().next().eq(&Some('0')) => {
                Ok(InputString::ValidNumber(u32::from_str_radix(s, 10)?))
            }
            Ok(_) => Ok(InputString::Garbage),
            Err(_) => Ok(InputString::Garbage),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    async fn terminate_cmd_test() {
        let input: [u8; 10] = "terminate\n"
            .as_bytes()
            .try_into()
            .expect("incorrect length");
        assert_eq!(parse_input(input).await.unwrap(), InputString::Termination);
    }

    #[tokio::test]
    async fn valid_data_test() {
        let input: [u8; 10] = "002345678\n"
            .as_bytes()
            .try_into()
            .expect("incorrect length");
        assert_eq!(
            parse_input(input).await.unwrap(),
            InputString::ValidNumber(2345678)
        );
    }

    #[tokio::test]
    async fn garbage_test() {
        let input: [u8; 10] = "912345678\n"
            .as_bytes()
            .try_into()
            .expect("incorrect length");
        assert_eq!(parse_input(input).await.unwrap(), InputString::Garbage);
    }

    #[tokio::test]
    async fn wo_eol_sym_test() {
        let input: [u8; 10] = "9123456789"
            .as_bytes()
            .try_into()
            .expect("incorrect length");
        assert_eq!(parse_input(input).await.unwrap(), InputString::Garbage);
    }

    #[tokio::test]
    async fn parsing_err_test() {
        let input: [u8; 10] = "0aawwweee\n"
            .as_bytes()
            .try_into()
            .expect("incorrect length");
        assert!(parse_input(input).await.is_err());
    }
}
