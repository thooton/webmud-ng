use std::net::IpAddr;

use anyhow::{Result, Context, bail};
use lazy_static::lazy_static;
use regex::Regex;
use tokio::{net::{TcpListener, TcpStream, tcp::{OwnedWriteHalf, OwnedReadHalf}}, io::{AsyncReadExt, AsyncWriteExt, BufReader, AsyncBufReadExt}};

pub fn start(host: IpAddr, port: u16) {
    let host = host.to_string();
    tokio::spawn(async move {
        Box::pin(go(format!("{}:{}", host, port))).await;
    });
}

async fn go(addr: String) {
    let server = TcpListener::bind(addr).await.unwrap();
    loop {
        if let Ok((stream, addr)) = server.accept().await {
            debug!("Accepted legacy WebSocket connection from {}", addr);
            tokio::spawn(async move {
                match Box::pin(handshake(stream)).await {
                    Ok(_) => {
                        debug!("Legacy WebSocket handshake from {} completed OK", addr);
                    },
                    Err(err) => {
                        debug!("Legacy WebSocket handshake from {} failed: {}", addr, err);
                    }
                }
            });
        }
    }
}

async fn handshake(mut stream: TcpStream) -> Result<()> {
    let mut buf = Vec::with_capacity(512);
    unsafe { buf.set_len(512); };
    let mut index = 0;
    loop {
        {
            let read = match stream.read(&mut buf[index..]).await {
                Ok(n) => n,
                Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                other_err => other_err?
            };
            // Client closed connection.
            if read == 0 {
                bail!("Client closed connection");
            }
            index += read;
            // Exhausted buffer. Nothing left to do.
            if index >= buf.len() {
                bail!("Exhausted buffer");
            }
        }
        let msg = String::from_utf8_lossy(&buf).to_string();
        if let Some((req, key_3)) = (|| -> Option<(&str, &[u8])> {
            let req_start = msg.find("GET / HTTP/")?;
            let header_end = {
                let area = msg.get(req_start..)?;
                area.find("\r\n\r\n").or(area.find("\n\n"))
            }? + req_start;
            let bytes_start = header_end + 4;
            let bytes_end = bytes_start + 8;
            Some((&msg.get(req_start..header_end)?, &buf.get(bytes_start..bytes_end)?))
        })() {
            // Received full client request
            lazy_static! {
                static ref KEY_1: Header = Header::new("Sec-WebSocket-Key1");
                static ref KEY_2: Header = Header::new("Sec-WebSocket-Key2");
                static ref HOST: Header = Header::new("Host");
                static ref ORIGIN: Header = Header::new("Origin");
            }

            let key_1 = KEY_1.try_get_in(&req)?;
            let key_2 = KEY_2.try_get_in(&req)?;
            let host = HOST.try_get_in(&req)?;
            let origin = ORIGIN.try_get_in(&req)?;

            let key_number_1: u64 = key_1.replace(|c: char| {
                !c.is_numeric()
            }, "").parse().unwrap();
            let key_number_2: u64 = key_2.replace(|c: char| {
                !c.is_numeric()
            }, "").parse().unwrap();

            let spaces_1 = key_1.replace(|c: char| {
                (c as u32) != 32
            }, "").len() as u64;
            let spaces_2 = key_2.replace(|c: char| {
                (c as u32) != 32
            }, "").len() as u64;

            if (spaces_1 == 0) 
            || (spaces_2 == 0) 
            || (key_number_1 % spaces_1 != 0) 
            || (key_number_2 % spaces_2 != 0) {
                stream.shutdown().await.unwrap();
                bail!("Incorrect client data");
            }

            let part_1 = (key_number_1 / spaces_1) as u32;
            let part_2 = (key_number_2 / spaces_2) as u32;

            let mut challenge = Vec::new();
            challenge.extend_from_slice(&part_1.to_be_bytes());
            challenge.extend_from_slice(&part_2.to_be_bytes());
            challenge.extend_from_slice(key_3);

            let response = md5::compute(challenge).0;

            let mut finish = Vec::new();
            finish.extend_from_slice(format!("{}\r\n{}\r\n{}\r\n{}\r\n{}\r\n\r\n", 
                "HTTP/1.1 101 WebSocket Protocol Handshake",
                "Upgrade: WebSocket",
                "Connection: Upgrade",
                format!("Sec-WebSocket-Origin: {}", origin),
                format!("Sec-WebSocket-Location: ws://{}/", host)
            ).as_bytes());
            finish.extend_from_slice(&response);
            stream.write_all(&finish).await?;

            tokio::spawn(async move {
                let addr = stream.peer_addr();
                let res = Box::pin(connection(stream)).await;
                if let Ok(addr) = addr {
                    match res {
                        Ok(_) => {
                            debug!("Legacy WebSocket connection from {} completed OK", addr);
                        },
                        Err(err) => {
                            debug!("Legacy WebSocket connection from {} completed with error: {}", addr, err);
                        }
                    }
                }
            });
            break;
        }
    }
    Ok(())
}

struct Header {
    regex: Regex
}

impl Header {
    fn new(header_name: &str) -> Self {
        Self {
            regex: Regex::new(
                &format!("(?im)^{}: (.*)$", header_name)
            ).unwrap()
        }
    }
    fn try_get_in<'a>(&self, text: &'a str) -> Result<&'a str> {
        Ok(
            self.regex.captures(&text).context("Header not found")?
            .get(1).context("Capture group error")?
            .as_str()
            .trim()
        )
    }
}

use tokio::sync::mpsc;

use crate::{conn::{ConnParent, self}, debug};

enum SocketMsg {
    Send(String),
    Close
}

#[derive(Clone)]
struct LegacyParent {
    tx: mpsc::UnboundedSender<SocketMsg>
}

impl ConnParent for LegacyParent {
    fn try_close_conn(&self) -> () {
        self.tx.send(SocketMsg::Close).ok();
    }
    fn try_send(&self, msg: String) -> () {
        self.tx.send(SocketMsg::Send(msg)).ok();
    }
}

async fn connection(stream: TcpStream) -> Result<()> {

    let (tx, rx) 
        = mpsc::unbounded_channel();
    //conn.init(LegacyParent { tx });
    let tx = conn::start(LegacyParent { tx });

    let (ws_read, ws_write) = new_ws(stream);

    let (close_notif_tx, close_notif_rx) = mpsc::channel(1);
    
    {
        let close_notif_tx = close_notif_tx;
        let mut ws_write = ws_write;
        let mut rx = rx;
        tokio::spawn(async move {
            loop {
                match rx.recv().await {
                    Some(SocketMsg::Send(to_send)) => {
                        if ws_write.send(&to_send).await.is_err() {
                            break;
                        };
                    },
                    Some(SocketMsg::Close) | None => {
                        ws_write.close().await;
                        close_notif_tx.send(true).await.ok();
                    }
                }
            }
        });
    }
    
    {
        let mut close_notif_rx = close_notif_rx;
        let mut ws_read = ws_read;
        let tx = tx;
        loop {
            tokio::select! {
                received = ws_read.recv() => {
                    match received {
                        Ok(msg) => {
                            tx.send(msg)?
                        },
                        error => {
                            error?;
                        }
                    }
                },
                _ = close_notif_rx.recv() => {
                    break;
                }
            };
        }
    }
    

    Ok(())
}

fn new_ws(stream: TcpStream) -> (WsRead, WsWrite) {
    let (read, write) = stream.into_split();
    (WsRead { 
        reader: BufReader::with_capacity(2048, read),
        buf: Vec::with_capacity(2048)
    },
    WsWrite {
        writer: write
    })
}

struct WsWrite {
    writer: OwnedWriteHalf
}

impl WsWrite {
    async fn send(&mut self, data: &str) -> Result<()> {
        let mut buf = Vec::with_capacity(data.len() + 2);
        buf.push(0x00);
        buf.extend_from_slice(data.as_bytes());
        buf.push(0xFF);

        self.writer.write_all(&buf).await?;
        Ok(())
    }
    async fn close(&mut self) {
        self.writer.write_all(&[0xFF, 0x00]).await.ok();
        self.writer.shutdown().await.ok();
    }
}

struct WsRead {
    reader: BufReader<OwnedReadHalf>,
    buf: Vec<u8>
}

impl WsRead {
    async fn recv(&mut self) -> Result<String> {
        read_until_with_max(&mut self.reader, 0xFF, &mut self.buf, 2048).await?;
        if self.buf.is_empty() {
            bail!("Connection closed");
        }

        if self.buf[0] == 0xFF {
            // Connection close request, OR different frame type.
            // The latter is not supported, so fail either way.
            bail!("Connection closed");
        } else if self.buf[0] != 0x00 {
            // Something went wrong
            bail!("Malformed frame");
        }

        let data = self.buf.get(1..self.buf.len() - 1).context("??")?;
        Ok(String::from_utf8_lossy(data).to_string())
    }
    
}

/*GET /chat/ HTTP/1.1
Upgrade: WebSocket
Connection: Upgrade
Host: 192.168.1.2:10108
Origin: http://192.168.1.2:10108
Sec-WebSocket-Key1: Um. X1 99P155 27f 20
Sec-WebSocket-Key2: 2 1l z80520%08S  q 0|"

I�Bgi�_ */

async fn read_until_with_max(r: &mut BufReader<OwnedReadHalf>, delim: u8, buf: &mut Vec<u8>, max: usize) 
    -> Result<()> {
    buf.clear();
    loop {
        let (done, used) = {
            let available = match r.fill_buf().await {
                Ok(n) => n,
                Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                other_err => other_err?
            };
            match memchr::memchr(delim, available) {
                Some(i) => {
                    if (buf.len() + i + 1) > max {
                        bail!("Limit reached");
                    }
                    buf.extend_from_slice(&available[..=i]);
                    (true, i + 1)
                }
                None => {
                    if (buf.len() + available.len()) > max {
                        bail!("Limit reached");
                    }
                    buf.extend_from_slice(available);
                    (false, available.len())
                }
            }
        };
        r.consume(used);
        if done || used == 0 {
            return Ok(());
        }
    }
}