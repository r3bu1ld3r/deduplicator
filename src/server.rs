use crate::storage::Storage;
use anyhow::{anyhow, Result};
use log::debug;
use std::{
    sync::{
        atomic::{AtomicBool, Ordering::SeqCst},
        Arc,
    },
    time::Duration,
};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::{broadcast, Semaphore},
    time::sleep,
};

pub struct DeDupServer {
    listener: TcpListener,
    conn_limit: Arc<Semaphore>,
    storage: Arc<Storage>,
    termination_notify: broadcast::Sender<()>,
    writer_shutdown: Arc<AtomicBool>,
    stats_collector: StatsCollector,
}

const MAX_CONNECTIONS: usize = 5;

impl DeDupServer {
    pub fn new(listener: TcpListener) -> Result<Self> {
        let conn_limit = Arc::new(Semaphore::new(MAX_CONNECTIONS));
        let writer_shutdown = Arc::new(AtomicBool::new(false));
        let storage = Arc::new(Storage::new(Arc::clone(&writer_shutdown))?);
        let (termination_notify, _) = broadcast::channel::<()>(1);
        let stats_collector =
            StatsCollector::new(Arc::clone(&storage), Arc::clone(&writer_shutdown));

        Ok(Self {
            listener,
            conn_limit,
            storage,
            termination_notify,
            writer_shutdown,
            stats_collector,
        })
    }

    pub async fn run(&self) -> Result<()> {
        self.stats_collector.run();
        let mut shutdown = self.termination_notify.subscribe();
        loop {
            tokio::select! {
                res = self.listener.accept(), if self.conn_limit.available_permits() > 0 => {
                    self.conn_limit.acquire().await?.forget();
                    let (stream, _) = res?;
                    let handler = ClientHandler::new(
                        stream,
                        Arc::clone(&self.conn_limit),
                        Arc::clone(&self.storage),
                        self.termination_notify.clone(),
                    );

                    tokio::spawn(async move {
                        if let Err(e) = handler.run().await {
                            debug!("Error during clients data processing: {e}");
                        };

                    });
                },
                _ = shutdown.recv() => {
                    while self.conn_limit.available_permits() != MAX_CONNECTIONS {
                        debug!("available permits: {}", self.conn_limit.available_permits());
                        sleep(Duration::from_millis(50)).await;
                    };
                    debug!("[+] all client connections closed");
                    self.writer_shutdown.store(true, std::sync::atomic::Ordering::SeqCst);
                    return Ok(())
                },
            };
        }
    }
}

pub(crate) struct ClientHandler {
    stream: TcpStream,
    conn_limit: Arc<Semaphore>,
    storage: Arc<Storage>,
    terminate_tx: broadcast::Sender<()>,
}

impl ClientHandler {
    pub(crate) fn new(
        stream: TcpStream,
        conn_limit: Arc<Semaphore>,
        storage: Arc<Storage>,
        terminate_tx: broadcast::Sender<()>,
    ) -> Self {
        Self {
            stream,
            conn_limit,
            storage,
            terminate_tx,
        }
    }

    pub(crate) async fn run(&self) -> Result<()> {
        loop {
            let mut shutdown = self.terminate_tx.subscribe();
            tokio::select! {
                res = self.stream.readable() => {
                    res?;
                    match self.recv_numbers().await {
                        Ok(r) if r == true => continue,
                        Ok(_) => {
                            debug!("[+] termination received from CURRENT client");
                            return Ok(())
                        },
                        Err(e) => {
                            debug!("error during recv_number: {e}");
                            return Ok(())
                        },
                    }
                    //TODO: actions for gracefull shutdown received from this client
                },
                _ = shutdown.recv() => {
                    debug!("[+] shutdown signal receieved from ANOTHER client");
                    return Ok(())
                },
            }
        }
    }

    async fn recv_numbers(&self) -> Result<bool> {
        let line = self.read_socket().await?;
        match ClientHandler::parse_input(line)? {
            InputString::ValidNumber(n) => {
                self.storage.append(n).await;
            }
            InputString::Termination | InputString::Garbage => {
                debug!(
                    "[+] active receivers: {}",
                    self.terminate_tx.receiver_count()
                );
                self.terminate_tx.send(())?;
                return Ok(false);
            }
        };
        Ok(true)
    }

    async fn read_socket(&self) -> Result<[u8; 10]> {
        let mut input_buf = [0; 10];
        self.stream.readable().await?;
        match self.stream.try_read(&mut input_buf) {
            Ok(n) if n == 0 => Err(anyhow!("stream closed by client")), //stream closed by client
            Ok(_) => Ok(input_buf), //TODO: maybe add case when have to read again if read less than bufsize
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                //false positive readiness - sleep and retry
                sleep(Duration::from_millis(10)).await;
                if let Err(e) = self.stream.try_read(&mut input_buf) {
                    Err(anyhow!("{e}"))
                } else {
                    Ok(input_buf)
                }
            },
            Err(e) => Err(anyhow!("{e}")),
        }
        
    }

    fn parse_input(input: [u8; 10]) -> Result<InputString> {
        if input[9] != 0xA {
            debug!("Garbage - without endline symbol");
            Ok(InputString::Garbage)
        } else {
            match std::str::from_utf8(&input[0..9]) {
                Ok(s) if s.eq("terminate") => Ok(InputString::Termination),
                Ok(s) if s.chars().next().eq(&Some('0')) => {
                    Ok(InputString::ValidNumber(s.parse::<u32>()?))
                }
                Ok(s) => {
                    debug!("garbage string: {s}");
                    Ok(InputString::Garbage)
                }
                Err(e) => {
                    debug!("Error during parsing from_utf8: {e}");
                    Ok(InputString::Garbage)
                }
            }
        }
    }
}

impl Drop for ClientHandler {
    fn drop(&mut self) {
        self.conn_limit.add_permits(1);
    }
}

pub(crate) struct StatsCollector {
    storage_handler: Arc<Storage>,
    terminate: Arc<AtomicBool>,
}

impl StatsCollector {
    pub(crate) fn new(storage: Arc<Storage>, terminate: Arc<AtomicBool>) -> Self {
        Self {
            storage_handler: storage,
            terminate,
        }
    }

    pub(crate) fn run(&self) {
        let handler_clone = Arc::clone(&self.storage_handler);
        let terminate_clone = Arc::clone(&self.terminate);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(10));
            loop {
                interval.tick().await;
                if !terminate_clone.load(SeqCst) {
                    handler_clone.print_stats().await;
                } else {
                    return;
                }
            }
        });
    }
}

#[derive(Debug, PartialEq)]
pub enum InputString {
    ValidNumber(u32),
    Termination,
    Garbage,
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
        assert_eq!(
            ClientHandler::parse_input(input).unwrap(),
            InputString::Termination
        );
    }

    #[tokio::test]
    async fn valid_data_test() {
        let input: [u8; 10] = "002345678\n"
            .as_bytes()
            .try_into()
            .expect("incorrect length");
        assert_eq!(
            ClientHandler::parse_input(input).unwrap(),
            InputString::ValidNumber(2345678)
        );
    }

    #[tokio::test]
    async fn garbage_test() {
        let input: [u8; 10] = "912345678\n"
            .as_bytes()
            .try_into()
            .expect("incorrect length");
        assert_eq!(
            ClientHandler::parse_input(input).unwrap(),
            InputString::Garbage
        );
    }

    #[tokio::test]
    async fn wo_eol_sym_test() {
        let input: [u8; 10] = "9123456789"
            .as_bytes()
            .try_into()
            .expect("incorrect length");
        assert_eq!(
            ClientHandler::parse_input(input).unwrap(),
            InputString::Garbage
        );
    }

    #[tokio::test]
    async fn parsing_err_test() {
        let input: [u8; 10] = "0aawwweee\n"
            .as_bytes()
            .try_into()
            .expect("incorrect length");
        assert!(ClientHandler::parse_input(input).is_err());
    }
}
