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

pub struct DeDupServer {
    listener: TcpListener,
    conn_limit: Arc<Semaphore>,
    storage: Arc<Mutex<Storage>>,
    termination_notify: broadcast::Sender<()>,
    //shutdown_complete_rx: mpsc::Receiver<()>,
    //shutdown_complete_tx: mpsc::Sender<()>,
}

pub(crate) struct ClientHandler{
    stream: TcpStream,
    conn_limit: Arc<Semaphore>,
    storage: Arc<Mutex<Storage>>,
    terminate_tx: broadcast::Sender<()>,
    terminate_rx: broadcast::Receiver<()>
}

impl ClientHandler{
    pub(crate) fn new(stream: TcpStream, conn_limit: Arc<Semaphore>, storage: Arc<Mutex<Storage>>, terminate_tx: broadcast::Sender<()>, terminate_rx: broadcast::Receiver<()>) -> Self {
        Self {
            stream,
            conn_limit,
            storage,
            terminate_tx,
            terminate_rx
        }
    }

    pub(crate) async fn run(&mut self) -> Result<()> {
        if let Ok(_guard) = self.conn_limit.try_acquire() {
            let mut watcher = true;
            while watcher {
                watcher = tokio::select! {
                    res = self.recv_numbers() => res?,
                    _ = self.terminate_rx.recv() => false,
                }
            }
        };
        Ok(())
    }

    async fn recv_numbers(&self) -> Result<bool> {
        Ok(true)
    }

    async fn read_socket(&self) -> Result<[u8; 10]> {
        let mut input_buf = [0; 10];
        loop{
            self.stream.readable().await?;
            match self.stream.try_read(&mut input_buf){
                Ok(n) if n == 0 => return Err(anyhow!("stream closed by client")), //stream closed by client
                Ok(_) => break,
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => continue,
                Err(e) => return Err(anyhow!("{e}"))
            }
        }
        Ok(input_buf)
    }
}

pub(crate) struct StatsCollector{
    storage_handler: Arc<Mutex<Storage>>
}

impl StatsCollector {
    pub(crate) fn new(storage: Arc<Mutex<Storage>>) -> Self {
        Self {
            storage_handler: storage
        } 
    }

    pub(crate) async fn run(&mut self) {
        let handler_clone = Arc::clone(&self.storage_handler);
        tokio::spawn(async move{
            let mut interval = tokio::time::interval(Duration::from_secs(10));
            loop{
                interval.tick().await;
                let storage = handler_clone.lock().await;
                storage.print_stats().await;
            }
        });       
    }
}

const MAX_CONNECTIONS: usize = 5;

impl DeDupServer{
    pub fn new(listener: TcpListener) -> Result<Self> {
        let conn_limit = Arc::new(Semaphore::new(MAX_CONNECTIONS));
        let storage = Arc::new(Mutex::new(Storage::new()?));
        let (termination_notify, _) = broadcast::channel::<()>(1);

        Ok(Self{
            listener,
            conn_limit,
            storage,
            termination_notify,
        })
    }

    pub async fn run(&self) -> Result<()> {
        let mut stats_collector = StatsCollector::new(Arc::clone(&self.storage));
        stats_collector.run().await;
        loop{
            let (stream, _) = self.listener.accept().await?;
            let handler = ClientHandler::new(
                stream,
                Arc::clone(&self.conn_limit),
                Arc::clone(&self.storage),
                self.termination_notify.clone(),
                self.termination_notify.subscribe()
            );

            tokio::spawn(async move {
                handler.run().await.unwrap()
            });
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum InputString {
    ValidNumber(u32),
    Termination,
    Garbage,
}

pub(crate) async fn read_number(
    stream: &mut TcpStream,
    storage: &Arc<Mutex<Storage>>,
    sender: &Sender<()>,
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
