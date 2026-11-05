//! Exercise TLS bootstrap in an isolated process before any SDK requests.
//! Reqwest 0.13 no-provider clients and Lexe's implicit rustls builder must
//! agree on a provider without depending on another adapter running first.

#[test]
fn http_and_sdk_tls_clients_can_be_constructed_from_a_cold_process() {
    let _http = lni::http_client_builder()
        .build()
        .expect("HTTP client must initialize its TLS provider");
    let _sdk_config = rustls::ClientConfig::builder()
        .with_root_certificates(rustls::RootCertStore::empty())
        .with_no_client_auth();
}
