use anyhow::Context as _;

use re_log_types::LogMsg;
use re_smart_channel::Sender;

/// Non-blocking.
#[allow(clippy::needless_pass_by_value)] // false positive on some feature flags
pub fn load_file_path(
    store_id: re_log_types::StoreId,
    path: std::path::PathBuf,
    tx: Sender<LogMsg>,
) -> anyhow::Result<()> {
    re_tracing::profile_function!(path.to_string_lossy());
    re_log::info!("Loading {path:?}…");

    if !path.exists() {
        anyhow::bail!("Failed to find file {path:?}.");
    }

    let extension = path
        .extension()
        .unwrap_or_default()
        .to_ascii_lowercase()
        .to_string_lossy()
        .to_string();

    if extension == "rrd" {
        stream_rrd_file(path, tx)
    } else {
        #[cfg(feature = "sdk")]
        {
            rayon::spawn(move || {
                if let Err(err) = load_and_send(&path, store_id, &tx) {
                    re_log::error!("Failed to load {path:?}: {err}");
                }
            });
            Ok(())
        }

        #[cfg(not(feature = "sdk"))]
        {
            _ = store_id;
            anyhow::bail!("Unsupported file extension: '{extension}' for path {path:?}. Try enabling the 'sdk' feature of 'rerun'.");
        }
    }
}

#[cfg(feature = "sdk")]
fn load_and_send(
    path: &std::path::PathBuf,
    store_id: re_log_types::StoreId,
    tx: &Sender<LogMsg>,
) -> anyhow::Result<()> {
    re_tracing::profile_function!(path.display().to_string());

    use re_log_types::SetStoreInfo;

    // First, set a store info since this is the first thing the application expects.
    tx.send(LogMsg::SetStoreInfo(SetStoreInfo {
        row_id: re_log_types::RowId::random(),
        info: re_log_types::StoreInfo {
            application_id: re_log_types::ApplicationId(path.display().to_string()),
            store_id: store_id.clone(),
            is_official_example: false,
            started: re_log_types::Time::now(),
            store_source: re_log_types::StoreSource::FileFromCli {
                rustc_version: env!("RE_BUILD_RUSTC_VERSION").into(),
                llvm_version: env!("RE_BUILD_LLVM_VERSION").into(),
            },
            store_kind: re_log_types::StoreKind::Recording,
        },
    }))
    .ok();
    // .ok(): we may be running in a background thread, so who knows if the receiver is still open

    // Send actual file.
    let msg_sender = re_sdk::MsgSender::from_file_path(path)?;
    let log_msg = msg_sender.into_log_msg(store_id)?;
    tx.send(log_msg).ok();
    tx.quit(None).ok();
    Ok(())
}

// Non-blocking
#[cfg(not(target_arch = "wasm32"))]
fn stream_rrd_file(
    path: std::path::PathBuf,
    tx: re_smart_channel::Sender<LogMsg>,
) -> anyhow::Result<()> {
    let file = std::fs::File::open(&path).context("Failed to open file")?;
    let decoder = re_log_encoding::decoder::Decoder::new(file)?;

    rayon::spawn(move || {
        re_tracing::profile_scope!("stream_rrd_file");
        for msg in decoder {
            match msg {
                Ok(msg) => {
                    tx.send(msg).ok(); // .ok(): we're running in a background thread, so who knows if the receiver is still open
                }
                Err(err) => {
                    re_log::warn_once!("Failed to decode message in {path:?}: {err}");
                }
            }
        }
        tx.quit(None).ok(); // .ok(): we're running in a background thread, so who knows if the receiver is still open
    });

    Ok(())
}
