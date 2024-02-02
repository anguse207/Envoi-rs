mod config_loader;
mod tls;

use axum::response::Html;
use axum::routing::any;
use axum::Router;
use hyper::service::service_fn;
use hyper::{Request, StatusCode};

use futures_util::stream::StreamExt;
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;
use tower_http::services::ServeDir;
use std::net::SocketAddr;
use std::{future::ready, sync::Mutex};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use hyper::server::conn::http1;
use hyper_util::rt::tokio::TokioIo;

use tls::tls_acceptor_impl;
use tls_listener::TlsListener;

use once_cell::sync::Lazy;
use tokio::net::TcpListener;

use config_loader::Config;

/*
TODO: 

- for servedir, tokio spawn an axum server and then the proxy service can route to it?!
- Create servedir, 404 from config

try and update to toml with format, these should be Option<T>
Maybe a seperate tls config file, 
as it seems we cannot use a different tls setting for the different hosts (yet).

[emby.citrusfire.co.uk]
destination = "http://192.168.68.100:8096"
serve_dir = "./www"

*/
// Load config from file / create new file
static HOSTS: Lazy<Config> = Lazy::new(Config::load);

static REQS: Lazy<Mutex<RequestsHandled>> = Lazy::new(|| Mutex::new(RequestsHandled::new()));

static CLIENT: Lazy<hyper_util::client::legacy::Client<HttpConnector, hyper::body::Incoming>> =
    Lazy::new(|| Client::builder(TokioExecutor::new()).build_http());

static HOST404: Lazy<String> = Lazy::new(|| "http://127.0.0.1:41050/".to_owned());

const CERT: &[u8] = include_bytes!("../res/tls/cloudflare-origin/public.der");
const PKEY: &[u8] = include_bytes!("../res/tls/cloudflare-origin/private.der");

struct RequestsHandled(u64);
impl RequestsHandled {
    fn increment(&mut self) {
        self.0 += 1;
    }
    fn print(&self) {
        tracing::info!("Requests Handled: {}\n", self.0);
    }
    fn new() -> Self {
        RequestsHandled(0)
    }
}

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

    let host = HOSTS.dest_map.get(host_header).unwrap_or(&HOST404);

    tracing::info!("{host_header} => {host}");

    let uri = format!("{host}{}", req.uri());

    *req.uri_mut() = uri.parse().unwrap();

    {
        let mut lock = REQS.lock().unwrap();
        lock.increment();
        lock.print();
        drop(lock)
    }

    CLIENT.request(req).await
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

    let service_404_handle = tokio::spawn(async { 
        create_404_service().await 
    });
    let service_dir_handle = tokio::spawn(async { 
        create_servedir_service().await 
    });
    let service_main_handle = tokio::spawn(async { 
        create_proxy_server().await 
    });
    
    tokio::select!(
        _ = service_404_handle
        => {
            tracing::error!("404 service failed")
        }
        _ = service_dir_handle => {
            tracing::error!("Serve dir service failed")
        }
        _ = service_main_handle => {
            tracing::error!("Main proxy service failed")
        }
    )
}


async fn create_proxy_server() {
    let addr: SocketAddr = "0.0.0.0:443".parse().unwrap();

    tracing::info!("Starting tls tcp listener on {addr}");

    // This uses a filter to handle errors with connecting
    TlsListener::new(
        tls_acceptor_impl(PKEY, CERT),
        TcpListener::bind(addr).await.unwrap(),
    )
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
    }).await
}

// TODO: 404 service should take in a port, optionally redirect? etc...
async fn create_404_service() {

    let service_handle = tokio::spawn(async {
        // build our application with a route
        let app = Router::new()
            .route("/", any(handler_404))
            .route("/*0", any(handler_404));

        // run it
        let listener = tokio::net::TcpListener::bind("127.0.0.1:41050")
            .await
            .unwrap();
        tracing::info!("Starting 404 service");
        axum::serve(listener, app).await.unwrap();
    });

    _ = service_handle.await;
}

async fn handler_404() -> (StatusCode, Html<&'static str>) {
    tracing::info!("404 hit");
    (
        StatusCode::NOT_FOUND,
        Html("<h1>You've hit 404, this host and/or address leads to nowhere...</h1>"),
    )
}

// TODO: serve dir route
async fn create_servedir_service() {

    let service_handle = tokio::spawn(async {
        // build our application with a route
        let app = Router::new()
        .nest_service("", 
            ServeDir::new("../res/dirs/www"));

        // run it
        let listener = tokio::net::TcpListener::bind("127.0.0.1:41051")
            .await
            .unwrap();
        tracing::info!("Starting serve dir service");
        axum::serve(listener, app).await.unwrap();
    });

    _ = service_handle.await;
}