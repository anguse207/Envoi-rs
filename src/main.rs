mod config_loader;
mod tls;

use axum::{extract::{Host, OriginalUri, Request}, response::IntoResponse, routing::any, Router};
use axum_server::tls_rustls::RustlsConfig;

use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;
use std::net::SocketAddr;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use once_cell::sync::Lazy;

use config_loader::Config;

/*
TODO: use different cert key combo per host
Can client just use different certs, and return response? NO
*/

// Load config from file / create new file
static HOSTS: Lazy<Config> = Lazy::new(Config::load);

const PUBLIC: &[u8] = include_bytes!("../tls/cloudflare-origin/public.pem");
const PRIVATE: &[u8] = include_bytes!("../tls/cloudflare-origin/private.pem");

async fn handle(
    Host(host): Host,
    OriginalUri(path): OriginalUri,
    mut req: Request,
) -> impl IntoResponse {
    // tracing::debug!("req -> \n{:?}", req);
    //tracing::debug!("host -> \n{}", host);
    //tracing::debug!("path -> \n{}", path.path());

    let host = HOSTS.dest_map
        .get(&host).unwrap();

    let uri = format!("{host}{}", path.path());
    tracing::debug!("uri -> \n{}", uri);

    // let mut debug_uri = req.uri().to_string();
    // if debug_uri.len() > 20 {
    //     debug_uri = debug_uri[0..20].to_string() + "..."
    // };

    *req.uri_mut() = uri.parse().unwrap();
    let client = Client::builder(TokioExecutor::new()).build_http();
    
    tracing::debug!("req -> \n{:?}", req);

    client.request(req).await.unwrap()
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

    // configure certificate and private key used by https
    let config = RustlsConfig::from_pem(
        PUBLIC.to_vec(),
        PRIVATE.to_vec(),
    )
    .await
    .unwrap();

    let app = Router::new()
        .route("/*0", any(handle))
        .route("/", any(handle))
    ;

    // run https server
    tracing::debug!("listening on {}", addr);
    axum_server::bind_rustls(addr, config)
        .serve(app.into_make_service())
        .await
        .unwrap();
}
