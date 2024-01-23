mod config_loader;
mod tls;

use hyper::service::service_fn;
use hyper::Request;

use futures_util::stream::StreamExt;
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;
use hyper_util::server::conn::auto::Builder;
use pki_types::{CertificateDer, PrivateKeyDer};
use rustls::ServerConfig;
use tokio_rustls::TlsAcceptor;
use std::sync::Arc;
use std::{fs, io};
use std::{env, future::ready};
use std::net::{Ipv4Addr, SocketAddr};
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

async fn handle(
    mut req: Request<hyper::body::Incoming>,
) -> Result<hyper::Response<hyper::body::Incoming>, hyper_util::client::legacy::Error> {
    println!("{:?}", req);

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
    //tracing::info!("\n{host_header} => {} @ {debug_uri} \n{host}", req.method());

    *req.uri_mut() = uri.parse().unwrap();
    let client = Client::builder(TokioExecutor::new()).build_http();

    client.request(req).await
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {

    let addr: SocketAddr = "0.0.0.0:443".parse().unwrap();

    // Load public certificate.
    let certs = load_certs("/home/citrusfire/Rust/envoi/tls/rustls-testing/cert.pem")?;
    // Load private key.
    let key = load_private_key("/home/citrusfire/Rust/envoi/tls/rustls-testing/key.rsa")?;

    println!("Starting to serve on https://{}", addr);

    // Create a TCP listener via tokio.
    let incoming = TcpListener::bind(&addr).await?;

    // Build TLS configuration.
    let mut server_config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .map_err(|e| error(e.to_string()))?;
    server_config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec(), b"http/1.0".to_vec()];
    let tls_acceptor = TlsAcceptor::from(Arc::new(server_config));

    let service = service_fn(handle);

    loop {
        let (tcp_stream, _remote_addr) = incoming.accept().await?;

        let tls_acceptor = tls_acceptor.clone();
        tokio::spawn(async move {
            let tls_stream = match tls_acceptor.accept(tcp_stream).await {
                Ok(tls_stream) => tls_stream,
                Err(err) => {
                    eprintln!("failed to perform tls handshake: {err:#}");
                    return;
                }
            };
            if let Err(err) = Builder::new(TokioExecutor::new())
                .serve_connection(TokioIo::new(tls_stream), service)
                .await
            {
                eprintln!("failed to serve connection: {err:#}");
            }
        });
    }
}

// Load public certificate from file.
fn load_certs(filename: &str) -> io::Result<Vec<CertificateDer<'static>>> {
    // Open certificate file.
    let certfile = fs::File::open(filename)
        .map_err(|e| error(format!("failed to open {}: {}", filename, e)))?;
    let mut reader = io::BufReader::new(certfile);

    // Load and return certificate.
    rustls_pemfile::certs(&mut reader).collect()
}

// Load private key from file.
fn load_private_key(filename: &str) -> io::Result<PrivateKeyDer<'static>> {
    // Open keyfile.
    let keyfile = fs::File::open(filename)
        .map_err(|e| error(format!("failed to open {}: {}", filename, e)))?;
    let mut reader = io::BufReader::new(keyfile);

    // Load and return a single private key.
    rustls_pemfile::private_key(&mut reader).map(|key| key.unwrap())
}

fn error(err: String) -> io::Error {
    io::Error::new(io::ErrorKind::Other, err)
}