mod tls_cfg;
use hyper::{
    server::{accept, conn::AddrIncoming},
    service::make_service_fn,
};
use hyper::{service::service_fn, Body, Client, Request, Response, Server};

use futures_util::stream::StreamExt;
use std::convert::Infallible;
use std::{
    collections::HashMap,
    future::ready,
};
use tls_cfg::tls_acceptor_impl;
use tls_listener::TlsListener;

use once_cell::sync::Lazy;

/*
TODO: use different cert key combo per host
Can client just use different certs, and return response?
*/ 

static HOSTS: Lazy<HashMap<&str, &str>> = Lazy::new(get_hosts);

const CERT: &[u8] = include_bytes!("../tls/emby/public.der");
const PKEY: &[u8] = include_bytes!("../tls/emby/private.der");

fn get_hosts() -> HashMap<&'static str, &'static str> {
    let mut hosts: HashMap<&str, &str> = HashMap::new();
    hosts.insert("npm.citrusfire.co.uk",            "http://192.168.68.100:81");
    hosts.insert("emby.citrusfire.co.uk",           "http://192.168.68.100:8096");
    hosts.insert("plex.citrusfire.co.uk",           "http://192.168.68.100:32400");
    hosts.insert("radarr.citrusfire.co.uk",         "http://192.168.68.100:7878");
    hosts.insert("sonarr.citrusfire.co.uk",         "http://192.168.68.100:8989");
    hosts.insert("prowlarr.citrusfire.co.uk",       "http://192.168.68.100:9696");
    hosts.insert("transmission.citrusfire.co.uk",   "http://192.168.68.100:9091");
    hosts.insert("request.citrusfire.co.uk",        "http://192.168.68.100:8920");
    hosts.insert("git.citrusfire.co.uk",            "http://192.168.68.100:2080");

    hosts
}

async fn handle(mut req: Request<Body>) -> Result<Response<Body>, hyper::Error> {
    let host_header = req.headers().get("host").unwrap().to_str().unwrap();

    let host = HOSTS.get(host_header)
                                .unwrap_or_else(|| &host_header);

    //let host = "http://192.168.68.100:81";
    let uri = format!("{host}{}", req.uri());
    println!("\n{host_header} => \n{host}");
    *req.uri_mut() = uri.parse().unwrap();
    let client = Client::new();

    client.request(req).await
}

#[tokio::main]
async fn main() {
    let addr = ([0, 0, 0, 0], 443).into();

    let new_svc = make_service_fn(|_| async { Ok::<_, Infallible>(service_fn(handle)) });

let incoming = TlsListener::new(tls_acceptor_impl(PKEY, CERT)
    , AddrIncoming::bind(&addr).unwrap())
        .connections()
        .filter(|conn| {
            if let Err(err) = conn {
                eprintln!("Error: {:?}", err);
                ready(false)
            } else {
                ready(true)
            }
        });

    let server = Server::builder(accept::from_stream(incoming)).serve(new_svc);
    server.await.unwrap()
}