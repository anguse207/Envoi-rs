use std::sync::Arc;
use tokio_rustls::rustls::{Certificate, PrivateKey, ServerConfig};

pub type Acceptor = tokio_rustls::TlsAcceptor;

pub fn tls_acceptor_impl(cert_der: &[u8], key_der: &[u8]) -> Acceptor {
    let key = PrivateKey(cert_der.into());
    let cert = Certificate(key_der.into());
    Arc::new(
        ServerConfig::builder()
            .with_safe_defaults()
            .with_no_client_auth()
            .with_single_cert(vec![cert], key)
            .unwrap(),
    )
    .into()
}
