use std::net::{ToSocketAddrs, IpAddr};

use anyhow::{Result, Context, bail};
use regex::Regex;
use crate::ansi::ansi2html;
use crate::config::get_config;
use crate::debug;

pub trait ConnParent {
    fn try_send(&self, msg: String) -> ();
    fn try_close_conn(&self) -> ();
}

fn try_json(parent: &impl ConnParent, msg: impl SerJson) {
    parent.try_send(msg.serialize_json())
}

/// The `conn` is constantly listening for new messages on its receiver.
/// If you drop the sender returned by this function, `conn` will be dropped.
pub fn start(mut parent: impl ConnParent + Send + 'static) -> UnboundedSender<String> {
    let (tx, rx) 
        = mpsc::unbounded_channel();
    tokio::spawn(async move {
        if let Err(err) = Box::pin(handle_conn(&mut parent, rx)).await {
            debug!("Connection failed with: {}", err);
            try_json(&parent, ClientMessage {
                message: format!("<br>{}<br>", err)
            });
            parent.try_close_conn();
        } else {}
    });
    tx
}

pub async fn handle_conn(parent: &mut impl ConnParent, mut rx: UnboundedReceiver<String>) -> Result<()> {
    let (host, port, tls) = get_details(&mut rx).await?;
    try_json(parent, ClientMessage {
        message: format!("<br>Attempting to establish a {}connection with {}:{}<br>", 
            if tls { "TLS " } else { "" }, host, port)
    });
    Box::pin(telnet_handler(host, port, parent, rx, tls)).await?;
    Ok(())
}

pub async fn get_details(rx: &mut UnboundedReceiver<String>) -> Result<(String, u16, bool)> {
    let msg = rx.recv().await.context("Client disconnect")?;
    let mut parser = msg.split(" ");
    let cmd = parser.next().context("No command provided")?;
    if cmd == "PHUD:CONNECT" {
        let host = parser.next().context("Invalid host")?.to_string();
        let port = parser.next().context("Invalid port")?.parse()?;
        let tls = parser.next().context("Invalid TLS value (true, false)")?.parse()?;
        Ok((host, port, tls))
    } else {
        bail!("Command unimplemented");
    }
}

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::net::TcpStream;
use libtelnet_rs::Parser;
use libtelnet_rs::events::TelnetEvents;

fn get_ip_ensure_non_local(host: &str) -> Result<IpAddr> {
    let ip = format!("{}:443", host)
        .to_socket_addrs()?
        .next().context("Unable to resolve IP")?.ip();
    if !get_config().allow_private_connections {
        let equal_local_ip = if let Some(local_ip) = crate::localip::get() {
            ip.eq(local_ip)
        } else {
            false
        };
        if !ip.is_global() || equal_local_ip {
            bail!("The provided host cannot be globally routed");
        }
    };
    Ok(ip)
}

use tokio_native_tls::{TlsConnector, TlsStream};

enum MaybeTls {
    Normal(TcpStream),
    Tls(TlsStream<TcpStream>)
}

impl MaybeTls {
    async fn connect(host: &str, ip: &str, port: u16, tls: bool) -> Result<Self> {
        let socket = TcpStream::connect(format!("{}:{}", ip, port)).await?;
        if !tls {
            Ok(Self::Normal(socket))
        } else {
            let mut cx = tokio_native_tls::native_tls::TlsConnector::builder();
            if get_config().allow_invalid_tls {
                cx.danger_accept_invalid_certs(true);
                cx.danger_accept_invalid_hostnames(true);
            }
            let cx = cx.build()?;
            let cx = TlsConnector::from(cx);
            let socket = cx.connect(host, socket).await?;
            Ok(Self::Tls(socket))
        }
    }
    async fn read(&mut self, dest: &mut [u8]) -> Result<usize> {
        match self {
            MaybeTls::Normal(stream) => {
                Ok(stream.read(dest).await?)
            },
            MaybeTls::Tls(stream) => {
                Ok(stream.read(dest).await?)
            }
        }
    }
    async fn write_all(&mut self, src: &[u8]) -> Result<()> {
        match self {
            MaybeTls::Normal(stream) => {
                Ok(stream.write_all(src).await?)
            },
            MaybeTls::Tls(stream) => {
                Ok(stream.write_all(src).await?)
            }
        }
    } 
}

async fn telnet_handler(host: String, port: u16, parent: &mut impl ConnParent, mut rx: mpsc::UnboundedReceiver<String>, tls: bool) -> Result<()> {
    let ip = get_ip_ensure_non_local(&host)?;
    let mut conn = MaybeTls::connect(&host, &ip.to_string(), port, tls).await?;
    //let mut conn = TcpStream::connect(format!("{}:{}", host, port)).await?;
    
    let mut telnet = Parser::new();
    let mut buf = Vec::with_capacity(2048);
    unsafe { buf.set_len(2048); }

    loop {
        tokio::select! {
            bytes_read = conn.read(&mut buf) => {
                let bytes_read: usize = bytes_read?;
                if bytes_read == 0 {
                    bail!("Connection closed");
                }
                let events = telnet.receive(&buf[..bytes_read]);
                for event in events {
                    match event {
                        TelnetEvents::DataReceive(data) => {
                            let data = strip_telnet(
                                String::from_utf8_lossy(&data).to_string()
                            );
                            try_json(parent, ClientMessage {
                                message: data
                            });
                        },
                        TelnetEvents::DataSend(to_send) => {
                            conn.write_all(&to_send).await?;
                        },
                        _ => {}
                    }
                }
            },
            to_send = rx.recv() => {
                let to_send = to_send.context("Client connection disconnected")?;
                if let TelnetEvents::DataSend(to_send) = telnet.send_text(to_send.trim()) {
                    conn.write_all(&to_send).await?;
                }
            }
        };
    }
}

//use lazy_static::lazy_static;

/*static TELNET_COLORS: [&'static str; 29] = ["[0m","[00m","[1m","[3m","[4m","[7m","[9m","[22m","[23m","[24m","[29m","[30m","[31m","[32m","[33m","[34m","[35m","[36m","[37m","[39m","[40m","[41m","[42m","[43m","[44m","[45m","[46m","[47m","[49m"]; 
static TELNET_REPLS: [&'static str; 29] = ["</b></span>","</b></span>","<b>","","","<span class='tnc_inverse'>","","</b>","","","","<span class='tnc_black'>","<span class='tnc_red'>","<span class='tnc_green'>","<span class='tnc_yellow'>","<span class='tnc_blue'>","<span class='tnc_magenta'>","<span class='tnc_cyan'>","<span class='tnc_white'>","<span class='tnc_default'>","<span class='tnc_bg_black'>","<span class='tnc_bg_red'>","<span class='tnc_bg_green'>","<span class='tnc_bg_yellow'>","<span class='tnc_bg_blue'>","<span class='tnc_bg_magenta'>","<span class='tnc_bg_cyan'>","<span class='tnc_bg_white'>","<span class='tnc_bg_default'>"];*/

fn strip_telnet(mut the_item: String) -> String {
    the_item = the_item
        .replace("<", "&lt;")
        .replace(">", "&gt;")
        .replace("\t", "     ");

    /*lazy_static! {
        static ref TELNET_COMBO: Regex = Regex::new(r"(?i)\[(\d+);").unwrap();
    }
    TELNET_COMBO.replace_all("hello", |cap: &regex::Captures| {
        format!("{}", cap.get(0).unwrap().as_str())
    });
    while TELNET_COMBO.is_match(&the_item) {
        the_item = TELNET_COMBO.replace_all(&the_item, "[${1}m\x1B[").to_string();
    }
    for (telnet_color, telnet_rep) in TELNET_COLORS.iter().zip(TELNET_REPLS.iter()).map(|(a,b)| (*a, *b)) {
        the_item = the_item.replace(&format!("\x1B{}", telnet_color), 
            if get_config().no_color { "" } else { telnet_rep });
    }*/

    the_item = ansi2html(&the_item);

    the_item
        .replace("\x1B", "")
        .replace("\r\n", "<br>")
        .replace("\n\r", "<br>")
        .replace("\r", "<br>")
        .replace("\n", "<br>")
        .replace("\u{00FF}\u{00F9}", "<br>")
        .replace(char::is_control, "")
        .replace("_-SYSTEM: CHAT-_", "")
        .replace("`", "'")
}

use nanoserde::SerJson;

#[derive(SerJson)]
struct ClientMessage {
    message: String
}