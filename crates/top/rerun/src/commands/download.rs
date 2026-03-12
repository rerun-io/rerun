use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

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
    pub fn run(self, _tokio_runtime: &tokio::runtime::Handle) -> anyhow::Result<()> {
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

            // Register stored credentials for gRPC dataset segments.
            if let LogDataSource::RedapDatasetSegment { ref uri, .. } = data_source {
                connection_registry
                    .set_credentials(&uri.origin, re_redap_client::Credentials::Stored);
            }

            let on_auth_err: re_data_source::AuthErrorHandler = Arc::new(|uri, err| {
                re_log::error!(%uri, "Authentication error: {err}");
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

            let rx = data_source.stream_with_options(
                on_auth_err,
                &connection_registry,
                streaming_options,
            )?;

            eprintln!("Downloading {url}…");

            save_to_rrd(&rx, &output_path)?;

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
