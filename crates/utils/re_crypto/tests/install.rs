#![cfg(not(target_arch = "wasm32"))]
#![cfg(any(feature = "crypto-ring", feature = "crypto-aws-lc-rs"))]

#[test]
fn installs_the_selected_provider_and_is_idempotent() {
    #[cfg(feature = "crypto-aws-lc-rs")]
    let expected = rustls::crypto::aws_lc_rs::default_provider();
    #[cfg(not(feature = "crypto-aws-lc-rs"))]
    let expected = rustls::crypto::ring::default_provider();

    re_crypto::init();
    let installed = rustls::crypto::CryptoProvider::get_default().unwrap();
    assert!(std::ptr::eq(
        installed.secure_random,
        expected.secure_random
    ));

    std::thread::scope(|scope| {
        for _ in 0..8 {
            scope.spawn(re_crypto::init);
        }
    });
    assert!(std::sync::Arc::ptr_eq(
        installed,
        rustls::crypto::CryptoProvider::get_default().unwrap(),
    ));
}
