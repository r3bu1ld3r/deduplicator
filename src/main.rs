pub mod storage;
use std::sync::Arc;

use anyhow::Result;
use tokio::{
    net::{TcpListener, TcpStream},
    sync::{Semaphore, Mutex},
};
use storage::Storage;
#[derive(Debug, PartialEq)]
pub enum InputString {
    Data(u32),
    Termination,
    Garbage,
}

#[tokio::main]
async fn main() -> Result<()> {
    let listener = TcpListener::bind("127.0.0.1:4000").await?;
    let clients_limit = Arc::new(Semaphore::new(5));
    let storage = Arc::new(Mutex::new(Storage::new().await?));

    loop {
        let (socket, _) = listener.accept().await?;
        let limit_cln = Arc::clone(&clients_limit);
        let storage_cln = Arc::clone(&storage);

        tokio::spawn(async move {
            if let Ok(_guard) = limit_cln.try_acquire() {
                request_handler(socket, storage_cln).await.unwrap();
            };
        });
    }
}

pub async fn request_handler(socket: TcpStream, storage: Arc<Mutex<Storage>>) -> Result<()> {
    let mut buf = [0; 10];
    socket.readable().await?;
    socket.try_read(&mut buf)?;
    match parse_input(buf).await? {
        InputString::Data(d) => {
            let mut store = storage.lock().await;
            Ok(store.append(d).await?)
        },
        InputString::Termination => unimplemented!("without graceful shutdown"),
        InputString::Garbage => Ok(()),
    }
}

pub async fn parse_input(input: [u8; 10]) -> Result<InputString> {
    if input[9] != 0xA {
        Ok(InputString::Garbage)
    } else {
        match std::str::from_utf8(&input[0..9]) {
            Ok(s) if s.eq("terminate") => Ok(InputString::Termination),
            Ok(s) if s.chars().nth(0).eq(&Some('0')) => {
                Ok(InputString::Data(u32::from_str_radix(s, 10)?))
            }
            Ok(_) => Ok(InputString::Garbage),
            Err(_) => Ok(InputString::Garbage),
        }
    }
}

mod test {
    use std::num::ParseIntError;
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
            InputString::Data(2345678)
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
