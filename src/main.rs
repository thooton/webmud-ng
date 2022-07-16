#![feature(ip)]

use std::{env::Args, net::IpAddr};

use actix_web::{web, App, Error, HttpRequest, HttpResponse, HttpServer, HttpResponseBuilder, http::StatusCode};
use actix_web_actors::ws;
use actix_web_static_files::ResourceFiles;

include!(concat!(env!("OUT_DIR"), "/generated.rs"));

mod handler;
use config::get_config;
use handler::SocketHandler;

mod conn;

mod legacy;

mod config {
    use std::net::IpAddr;

    pub struct Config {
        pub ip: IpAddr,
        pub port: u16,
        pub no_color: bool,
        pub debug: bool,
        pub allow_private_connections: bool,
        pub allow_invalid_tls: bool,
        pub legacy_info: Option<(IpAddr, u16)>,
        pub legacy_extern_ip: Option<String>,
        pub legacy_extern_is_https: bool,
        pub legacy_extern_port: Option<u16>,
        pub extern_is_https: bool,
        pub legacy_only: bool,
        pub serve_from: Option<String>
    }
    static mut CONFIG: Option<Config> = None;
    pub unsafe fn set_config(config: Config) {
        CONFIG = Some(config);
    }
    pub fn get_config() -> &'static Config {
        unsafe { CONFIG.as_ref().unwrap() }
    }
    #[macro_export]
    macro_rules! debug {
        ($($arg:tt)*) => {{
            if crate::get_config().debug {
                println!($($arg)*);
            }
        }};
    }
}

async fn index() -> Result<HttpResponse, Error> {
    HttpResponse::MovedPermanently()
        .append_header(("Location", "index.html"))
        .await
}


async fn echo_ws(req: HttpRequest, stream: web::Payload) -> Result<HttpResponse, Error> {
    ws::start(SocketHandler::new(), &req, stream)
}

async fn dyn_vars() -> Result<HttpResponse, Error> {
    let Config { legacy_extern_port, legacy_extern_is_https, extern_is_https, legacy_extern_ip: legacy_extern_host, .. } = get_config();
    let legacy_extern_port = (*legacy_extern_port).unwrap_or(443);
    
    Ok(HttpResponseBuilder::new(StatusCode::OK)
        .content_type("application/javascript")
        .body(format!(r#"var WNG_LEGACY_CONNECTION_PORT = "{}"; var WNG_LEGACY_PREFIX = "{}"; var WNG_NORMAL_PREFIX = "{}"; var WNG_LEGACY_HOST = {};"#, 
            legacy_extern_port, 
            if *legacy_extern_is_https { "wss" } else { "ws" },
            if *extern_is_https { "wss" } else { "ws" },
            if let Some(leh) = legacy_extern_host { format!(r#""{}""#, leh) } else { "window.location.hostname".to_string() }
        )))
}

use crate::config::{Config, set_config};
use anyhow::Context;

fn flag_exists(args: &[String], flag: &str) -> bool {
    args.contains(&flag.to_string())
}

fn flag_value(args: &[String], flag: &str) -> Option<String> {
    let needle = format!("{}=", flag);
    for arg in args {
        if arg.starts_with(&needle) {
            let val = arg[needle.len()..].trim().to_string(); 
            if !val.is_empty() {
                return Some(val);
            }
        }
    }
    for arg in args {
        if arg.starts_with(flag) {
            eprintln!("The format of flags is --key=value");
            std::process::exit(1);
        }
    }
    None
}

fn parse_args(args: Args) -> anyhow::Result<Config> {
    let args: Vec<String> = args.map(|x| x.trim().to_string()).collect();
    if args.len() == 1 || args.contains(&"-h".to_string()) || args.contains(&"--help".to_string()) {
        eprintln!(
"Usage: webmud-ng <ip> <port> [--extern-is-https] [--legacy-only] [--legacy-ip=#] [--legacy-port=#] [--legacy-extern-host=#] [--legacy-extern-port=#] [--legacy-extern-is-https] [--no-color] [--serve-from=directory] [--allow-private-connections] [--allow-invalid-tls] [--debug]"
        );
        eprintln!("See webmud-ng GitHub for details");
        std::process::exit(0);
    }
    let mut args = args.into_iter();
    args.next();
    
    let ip: IpAddr = args.next().context("No IP provided")?.parse()?;
    let port: u16 = args.next().context("No port provided")?.parse()?;
    let rest: Vec<String> = args.collect();
    let no_color = flag_exists(&rest, "--no-color");
    let allow_private_connections = flag_exists(&rest, "--allow-private-connections");
    let allow_invalid_tls = flag_exists(&rest, "--allow-invalid-tls");
    let debug = flag_exists(&rest, "--debug");
    let legacy_info = {
        let legacy_ip = flag_value(&rest, "--legacy-ip");
        let legacy_port = flag_value(&rest, "--legacy-port");
        if legacy_ip.is_none() && legacy_port.is_none() {
            None
        } else if legacy_ip.is_some() ^ legacy_port.is_some() {
            anyhow::bail!("If legacy IP is specified, legacy port must be specified, and vice versa.");
        } else {
            let legacy_ip = legacy_ip.unwrap().parse()?;
            let legacy_port = legacy_port.unwrap().parse()?;
            Some((legacy_ip, legacy_port))
        }
    };
    let legacy_extern_port = flag_value(&rest, "--legacy-extern-port")
            .map(|x| x.parse())
            .transpose()?
            .or(legacy_info.map(|(_, legacy_port)| legacy_port));
    let serve_from = flag_value(&rest, "--serve-from");
    let legacy_extern_is_https = flag_exists(&rest, "--legacy-extern-is-https");
    let extern_is_https = flag_exists(&rest, "--extern-is-https");
    let legacy_extern_ip = flag_value(&rest, "--legacy-extern-host");
    let legacy_only = flag_exists(&rest, "--legacy-only");
    if legacy_only && legacy_info.is_none() {
        anyhow::bail!("If --legacy-only is set, legacy info (--legacy-ip, --legacy-port, optional --legacy-extern-port) must be specified.");
    }

    Ok(Config {
        ip,
        port,
        debug,
        no_color,
        allow_private_connections,
        legacy_extern_is_https,
        extern_is_https,
        allow_invalid_tls,
        legacy_info,
        serve_from,
        legacy_extern_ip,
        legacy_extern_port,
        legacy_only
    })
}

mod localip {
    use std::net::IpAddr;

    use local_ip_address::local_ip;

    static mut LOCAL_IP: Option<IpAddr> = None;
    
    pub unsafe fn init() {
        LOCAL_IP = local_ip().ok();
    }

    pub fn get() -> &'static Option<IpAddr> {
        unsafe { &LOCAL_IP }
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    #[cfg(unix)]
    openssl_probe::init_ssl_cert_env_vars();

    unsafe { localip::init(); }

    let config = parse_args(std::env::args()).unwrap_or_else(|err| {
        eprintln!("{}", err);
        std::process::exit(1);
    });
    unsafe { set_config(config) };
    let Config { ip, port, legacy_info, serve_from, legacy_extern_ip, legacy_extern_port, legacy_only, debug, .. } = get_config();

    if *debug {
        eprintln!("Debug mode enabled");
    }

    if !legacy_only {
        eprintln!("Listening at http://{}:{}", ip, port);
    }

    if let Some((ip, port)) = legacy_info {
        legacy::start(ip.clone(), *port);
        eprintln!("Listening for legacy WS connections at ws://{}:{} (extern {}:{})", ip, port, legacy_extern_ip.clone().unwrap_or("auto".to_string()), legacy_extern_port.unwrap());
    }

    if !legacy_only {
        if let Some(serve_path) = serve_from {
            eprintln!("Serving files dynamically from directory {}", serve_path);
        } else {
            eprintln!("Serving files statically from bundle in binary");
        }
        HttpServer::new(|| { 
            let generated = generate();
            let app = App::new()
                .service(web::resource("/").route(web::get().to(index)))
                .service(web::resource("/ws").route(web::get().to(echo_ws)))
                .route("/dyn_vars.js", web::get().to(dyn_vars));
            
            if let Some(serve_path) = serve_from.clone() {
                app.service(actix_files::Files::new("/", &serve_path))
            } else {
                app.service(ResourceFiles::new("/", generated))
            }
        })
        .bind((ip.clone(), *port))?
        .run()
        .await
    } else {
        tokio::signal::ctrl_c().await?;
        Ok(())
    }
}
