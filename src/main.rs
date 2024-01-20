mod config_loader;
mod tls;

use hyper::service::service_fn;
use hyper::Request;

use futures_util::stream::StreamExt;
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;
use std::future::ready;
use std::net::SocketAddr;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use hyper::server::conn::http1;
use hyper_util::rt::tokio::TokioIo;

use tls::tls_acceptor_impl;
use tls_listener::TlsListener;

use once_cell::sync::Lazy;
use tokio::net::TcpListener;

use config_loader::Config;

/*
TODO: use different cert key combo per host
Can client just use different certs, and return response? NO
*/

// Load config from file / create new file
static HOSTS: Lazy<Config> = Lazy::new(Config::load);

const CERT: &[u8] = include_bytes!("../tls/cloudflare-origin/public.der");
const PKEY: &[u8] = include_bytes!("../tls/cloudflare-origin/private.der");

async fn handle(
    mut req: Request<hyper::body::Incoming>,
) -> Result<hyper::Response<hyper::body::Incoming>, hyper_util::client::legacy::Error> {
    let host_header = &req
        .headers()
        .get("host")
        .unwrap()
        .to_str()
        .unwrap()
        .to_owned();

    let host = HOSTS.dest_map.get(host_header).unwrap_or(host_header);

    let uri = format!("{host}{}", req.uri());

    let mut debug_uri = req.uri().to_string();
    if debug_uri.len() > 20 {
        debug_uri = debug_uri[0..20].to_string() + "..."
    };
    tracing::info!("\n{host_header} => {} @ {debug_uri} \n{host}", req.method());

    *req.uri_mut() = uri.parse().unwrap();
    let client = Client::builder(TokioExecutor::new()).build_http();

    client.request(req).await
}

#[tokio::main]
async fn main() {
    // Create and start logger
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "envoi=trace,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let addr: SocketAddr = "0.0.0.0:443".parse().unwrap();

    tracing::info!("Starting Tls Tcp listener on {addr}");

    // This uses a filter to handle errors with connecting
    TlsListener::new(tls_acceptor_impl(PKEY,CERT), 
        TcpListener::bind(addr).await.unwrap())
        .connections()
        .filter_map(|conn| {
            ready(match conn {
                Err(err) => {
                    tracing::error!("{err}");
                    None
                }
                Ok(c) => Some(TokioIo::new(c)),
            })
        })
        .for_each_concurrent(None, |conn| async {
            if let Err(err) = http1::Builder::new()
                .serve_connection(conn, service_fn(handle))
                .await
            {
                eprintln!("Error serving connection: {:?}", err);
            }
        })
        .await;
}
