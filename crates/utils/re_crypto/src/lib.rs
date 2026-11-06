//! Shared provider selection for native TLS.
//!
//! Ring is enabled by default.
//! Disable default features and enable `crypto-aws-lc-rs` to use AWS-LC instead.
//! AWS-LC takes precedence if Cargo unifies both features.
//! The web uses the browser's TLS implementation and does not build either backend.

/// Install the selected TLS provider unless the application already installed one.
///
/// Call before creating a native HTTP or gRPC client.
/// Explicit selection also handles builds where Cargo enables both backends.
/// Safe to call repeatedly and concurrently; does nothing on the web.
pub fn init() {
    #[cfg(all(
        not(target_arch = "wasm32"),
        any(feature = "crypto-ring", feature = "crypto-aws-lc-rs")
    ))]
    {
        if rustls::crypto::CryptoProvider::get_default().is_some() {
            return;
        }

        #[cfg(feature = "crypto-aws-lc-rs")]
        let provider = rustls::crypto::aws_lc_rs::default_provider();
        #[cfg(not(feature = "crypto-aws-lc-rs"))]
        let provider = rustls::crypto::ring::default_provider();

        // Another thread or the embedding application may have installed it first.
        let _ignored = provider.install_default();
    }
}
