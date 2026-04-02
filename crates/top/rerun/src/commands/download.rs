use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use parking_lot::Mutex;

use re_data_source::{FromUriOptions, LogDataSource};
use re_log_channel::DataSourceMessage;

/// Download recordings and save them as `.rrd` files.
///
/// Supports any URI that Rerun can load: gRPC dataset segment URLs,
/// HTTP URLs to `.rrd` files, local file paths, etc.
#[derive(Debug, Clone, clap::Parser)]
pub struct DownloadCommand {
    /// One or more URIs to download.
    #[clap(required = true)]
    urls: Vec<String>,

    /// Override the output directory for the downloaded `.rrd` files.
    ///
    /// Defaults to the current working directory.
    #[clap(short, long)]
    output_dir: Option<std::path::PathBuf>,
}

impl DownloadCommand {
    pub fn run(self, tokio_runtime: &tokio::runtime::Handle) -> anyhow::Result<()> {
        let output_dir = self
            .output_dir
            .unwrap_or_else(|| std::path::PathBuf::from("."));

        if !output_dir.exists() {
            std::fs::create_dir_all(&output_dir)?;
        }

        let connection_registry =
            re_redap_client::ConnectionRegistry::new_with_stored_credentials();

        for url in &self.urls {
            let data_source = LogDataSource::from_uri(
                re_log_types::FileSource::Cli,
                url,
                &FromUriOptions {
                    follow: false,
                    accept_extensionless_http: true,
                },
            );

            let Some(data_source) = data_source else {
                anyhow::bail!("Could not interpret URI: {url}");
            };

            let output_path = output_dir.join(output_filename(&data_source, url));

            // For gRPC dataset segments, ensure credentials are valid before streaming.
            if let LogDataSource::RedapDatasetSegment { ref uri, .. } = data_source {
                ensure_credentials(tokio_runtime, &connection_registry, &uri.origin)?;
            }

            let auth_error: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
            let auth_error_capture = auth_error.clone();

            let on_auth_err: re_data_source::AuthErrorHandler = Arc::new(move |uri, err| {
                let msg = format!("Authentication failed for {uri}: {err}");
                *auth_error_capture.lock() = Some(msg);
            });

            let downloaded = Arc::new(AtomicU64::new(0));
            let downloaded_for_progress = downloaded.clone();

            let streaming_options = re_redap_client::StreamingOptions {
                force_full_download: true,
                on_progress: Some(Arc::new(move |bytes_downloaded, total_bytes| {
                    downloaded_for_progress.store(bytes_downloaded, Ordering::Relaxed);
                    match total_bytes {
                        Some(total) => {
                            let percent = if 0 < total {
                                100.0 * bytes_downloaded as f64 / total as f64
                            } else {
                                100.0
                            };
                            eprint!(
                                "\r  {:.1}% ({} / {})",
                                percent,
                                re_format::format_bytes(bytes_downloaded as _),
                                re_format::format_bytes(total as _),
                            );
                        }
                        None => {
                            eprint!("\r  {}", re_format::format_bytes(bytes_downloaded as _),);
                        }
                    }
                })),
            };

            let readable_url = data_source.as_uri().unwrap_or_else(|| url.clone());

            let rx = data_source.stream_with_options(
                on_auth_err,
                &connection_registry,
                streaming_options,
            )?;

            eprintln!("Downloading {readable_url}…");

            save_to_rrd(&rx, &output_path)?;

            // Check if the async streaming task encountered an auth error.
            if let Some(auth_err_msg) = auth_error.lock().take() {
                // Remove the (likely empty/incomplete) output file.
                std::fs::remove_file(&output_path).ok();
                anyhow::bail!(
                    "{auth_err_msg}\n\nRun `rerun auth login` to re-authenticate, then try again."
                );
            }

            let total = downloaded.load(Ordering::Relaxed);
            if 0 < total {
                // Clear the progress line
                eprint!("\r\x1b[2K");
            }

            eprintln!("Saved {output_path:?}");
        }

        Ok(())
    }
}

/// Ensure we have valid credentials for the given origin.
///
/// If stored credentials exist but are expired, this attempts to refresh them.
/// If the refresh fails (e.g. session ended), it triggers an interactive
/// device-code login flow so the user can re-authenticate.
#[cfg(feature = "auth")]
fn ensure_credentials(
    tokio_runtime: &tokio::runtime::Handle,
    connection_registry: &re_redap_client::ConnectionRegistryHandle,
    origin: &re_uri::Origin,
) -> anyhow::Result<()> {
    use re_auth::oauth::login_flow::DeviceCodeFlowState;

    match tokio_runtime.block_on(re_auth::oauth::load_and_refresh_credentials()) {
        Ok(Some(_credentials)) => {
            // Credentials are valid.
            connection_registry.set_credentials(origin, re_redap_client::Credentials::Stored);
        }

        Ok(None) => {
            // No stored credentials. Proceed without — the server may not require auth.
        }

        Err(err) => {
            re_log::debug!("Credential refresh failed: {err}");
            eprintln!("Session expired. Logging in again…");

            // Trigger interactive device-code login flow.
            match tokio_runtime.block_on(re_auth::DeviceCodeFlow::init(true)) {
                Ok(DeviceCodeFlowState::AlreadyLoggedIn(_)) => {
                    // Shouldn't happen with force_login=true, but handle it gracefully.
                    connection_registry
                        .set_credentials(origin, re_redap_client::Credentials::Stored);
                }

                Ok(DeviceCodeFlowState::LoginFlowStarted(mut flow)) => {
                    let login_url = flow.login_url();
                    let user_code = flow.user_code();

                    eprintln!("Open this URL in your browser to log in:\n  {login_url}");
                    eprintln!("Verify that the code shown in your browser is: {user_code}");
                    eprintln!("Waiting for login…");

                    match tokio_runtime.block_on(flow.wait_for_user_confirmation()) {
                        Ok(credentials) => {
                            eprintln!("Logged in as {}", credentials.user().email);
                            // Clear the cached client so a new one is created with fresh credentials.
                            connection_registry.remove_credentials(origin);
                            connection_registry
                                .set_credentials(origin, re_redap_client::Credentials::Stored);
                        }
                        Err(err) => {
                            anyhow::bail!(
                                "Login failed: {err}\n\nRun `rerun auth login` to authenticate manually."
                            );
                        }
                    }
                }

                Err(err) => {
                    anyhow::bail!(
                        "Could not start login flow: {err}\n\nRun `rerun auth login` to authenticate manually."
                    );
                }
            }
        }
    }

    Ok(())
}

#[cfg(not(feature = "auth"))]
fn ensure_credentials(
    _tokio_runtime: &tokio::runtime::Handle,
    connection_registry: &re_redap_client::ConnectionRegistryHandle,
    origin: &re_uri::Origin,
) -> anyhow::Result<()> {
    connection_registry.set_credentials(origin, re_redap_client::Credentials::Stored);
    Ok(())
}

/// Derive an output `.rrd` filename from the data source.
fn output_filename(data_source: &LogDataSource, original_url: &str) -> std::path::PathBuf {
    match data_source {
        LogDataSource::RedapDatasetSegment { uri, .. } => format!("{}.rrd", uri.segment_id).into(),

        #[cfg(not(target_arch = "wasm32"))]
        LogDataSource::FilePath { path, .. } => path
            .file_name()
            .map(Into::into)
            .unwrap_or_else(|| "output.rrd".into()),

        LogDataSource::HttpUrl { url, .. } => {
            let path = url.path();
            let filename = path.rsplit('/').next().unwrap_or("output.rrd");
            if filename.is_empty() {
                "output.rrd".into()
            } else if filename.ends_with(".rrd") || filename.ends_with(".rbl") {
                filename.into()
            } else {
                format!("{filename}.rrd").into()
            }
        }

        _ => {
            re_log::warn!("Cannot derive filename from {original_url:?}, using fallback");
            "output.rrd".into()
        }
    }
}

/// Receive all messages from the channel and write them to an `.rrd` file.
fn save_to_rrd(
    rx: &re_log_channel::LogReceiver,
    output_path: &std::path::Path,
) -> anyhow::Result<()> {
    let encoding_options = re_log_encoding::rrd::EncodingOptions::PROTOBUF_COMPRESSED;
    let file = std::fs::File::create(output_path)?;
    let mut encoder = re_log_encoding::Encoder::new_eager(
        re_build_info::CrateVersion::LOCAL,
        encoding_options,
        file,
    )?;

    while let Ok(msg) = rx.recv() {
        if let Some(payload) = msg.into_data() {
            match payload {
                DataSourceMessage::LogMsg(log_msg) => {
                    encoder.append(&log_msg)?;
                }
                other => {
                    re_log::trace!("Skipping {} (not storable in .rrd)", other.variant_name());
                }
            }
        }
    }

    encoder.finish()?;

    Ok(())
}
