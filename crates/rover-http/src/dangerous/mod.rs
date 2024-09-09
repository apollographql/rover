use std::sync::Arc;

use hyper_rustls::HttpsConnector;
use hyper_util::client::legacy::connect::HttpConnector;
use rustls::ClientConfig;

use self::tls::{IgnoreHostname, NoVerifier};

mod tls;

pub fn connector(
    accept_invalid_certs: bool,
    accept_invalid_hostnames: bool,
) -> HttpsConnector<HttpConnector> {
    let provider = rustls::crypto::CryptoProvider::get_default()
        .cloned()
        .unwrap_or_else(|| Arc::new(rustls::crypto::ring::default_provider()));
    let signature_verification_algorithms = provider.signature_verification_algorithms;
    let config_builder = ClientConfig::builder_with_provider(provider)
        .with_protocol_versions(rustls::ALL_VERSIONS)
        .expect("Invalid TLS version");
    let root_certs = rustls::RootCertStore::empty();
    let config_builder = if accept_invalid_certs {
        config_builder
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(NoVerifier))
    } else if accept_invalid_hostnames {
        config_builder
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(
                IgnoreHostname::builder()
                    .roots(root_certs)
                    .signature_algorithms(signature_verification_algorithms)
                    .build(),
            ))
    } else {
        config_builder.with_root_certificates(root_certs)
    };
    let config = config_builder.with_no_client_auth();
    HttpsConnector::<HttpConnector>::builder()
        .with_tls_config(config)
        .https_or_http()
        .enable_http1()
        .build()
}
