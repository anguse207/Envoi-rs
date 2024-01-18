pub mod tls_cfg {
    use std::sync::Arc;
    use tokio_rustls::rustls::{Certificate, PrivateKey, ServerConfig};

    const CERT: &[u8] = include_bytes!("../certs/public.der");
    const PKEY: &[u8] = include_bytes!("../certs/private.der");

    pub type Acceptor = tokio_rustls::TlsAcceptor;

    fn tls_acceptor_impl(cert_der: &[u8], key_der: &[u8]) -> Acceptor {
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

    pub fn tls_acceptor() -> Acceptor {
        tls_acceptor_impl(PKEY, CERT)
    }
}
