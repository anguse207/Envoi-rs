mod tls;
mod config_loader;

use hyper::{
    server::{accept, conn::AddrIncoming},
    service::make_service_fn,
};
use hyper::{service::service_fn, Body, Client, Request, Response, Server};

use futures_util::stream::StreamExt;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use std::{convert::Infallible, net::SocketAddr};
use std::{
    collections::HashMap,
    future::ready,
};
use tls::tls_acceptor_impl;
use tls_listener::TlsListener;

use once_cell::sync::Lazy;

use config_loader::{Config, Tls};

/*
TODO: use different cert key combo per host
Can client just use different certs, and return response?
*/ 


// Load config from file / create new file
static HOSTS: Lazy<Config> = Lazy::new(Config::load);

const CERT: &[u8] = include_bytes!("../tls/cloudflare-origin/public.der");
const PKEY: &[u8] = include_bytes!("../tls/cloudflare-origin/private.der");


async fn handle(mut req: Request<Body>) -> Result<Response<Body>, hyper::Error> {
    let host_header = &req.headers().get("host").unwrap().to_str().unwrap().to_owned();

    let host = HOSTS.dest_map.get(host_header)
                                .unwrap_or(host_header);

    let uri = format!("{host}{}", req.uri());

    tracing::info!("\n{host_header} => {} @ {} \n{host}", req.method(), req.uri());
    
    *req.uri_mut() = uri.parse().unwrap();
    let client = Client::new();
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

    let addr:SocketAddr = "0.0.0.0:443"
                            .parse().unwrap();

    let new_svc = 
        make_service_fn(|_| async { Ok::<_, Infallible>(service_fn(handle)) });

    tracing::info!("Starting Tls Tcp listener on {addr}");

    let incoming = TlsListener::new(tls_acceptor_impl(PKEY, CERT), 
        AddrIncoming::bind(&addr).unwrap())
            .connections()
            .filter(|conn| {
                if let Err(err) = conn {
                    tracing::error!("Error: {:?}", err);
                    ready(false)
                } else {
                    ready(true)
                }
        });

    let server = Server::builder(accept::from_stream(incoming)).serve(new_svc);
    server.await.unwrap()
}