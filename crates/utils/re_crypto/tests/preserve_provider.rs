#![cfg(not(target_arch = "wasm32"))]
#![cfg(any(feature = "crypto-ring", feature = "crypto-aws-lc-rs"))]

#[test]
fn preserves_the_embedding_applications_provider() {
    #[cfg(feature = "crypto-aws-lc-rs")]
    let mut provider = rustls::crypto::aws_lc_rs::default_provider();
    #[cfg(not(feature = "crypto-aws-lc-rs"))]
    let mut provider = rustls::crypto::ring::default_provider();

    provider.cipher_suites.truncate(1);
    provider.install_default().unwrap();
    let installed = rustls::crypto::CryptoProvider::get_default().unwrap();
    re_crypto::init();
    assert!(std::sync::Arc::ptr_eq(
        installed,
        rustls::crypto::CryptoProvider::get_default().unwrap(),
    ));
    assert_eq!(installed.cipher_suites.len(), 1);
}
