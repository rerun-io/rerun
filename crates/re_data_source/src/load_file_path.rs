use anyhow::Context as _;

use re_log_types::{FileSource, LogMsg};
use re_smart_channel::Sender;

use crate::load_file::data_cells_from_file_path;

/// Non-blocking.
#[allow(clippy::needless_pass_by_value)] // false positive on some feature flags
pub fn load_file_path(
    store_id: re_log_types::StoreId,
    file_source: FileSource,
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
        rayon::spawn(move || {
            if let Err(err) = load_and_send(store_id, file_source, &path, &tx) {
                re_log::error!("Failed to load {path:?}: {err}");
            }
        });
        Ok(())
    }
}

fn load_and_send(
    store_id: re_log_types::StoreId,
    file_source: FileSource,
    path: &std::path::Path,
    tx: &Sender<LogMsg>,
) -> anyhow::Result<()> {
    re_tracing::profile_function!(path.display().to_string());

    use re_log_types::SetStoreInfo;

    let store_source = re_log_types::StoreSource::File { file_source };

    // First, set a store info since this is the first thing the application expects.
    tx.send(LogMsg::SetStoreInfo(SetStoreInfo {
        row_id: re_log_types::RowId::new(),
        info: re_log_types::StoreInfo {
            application_id: re_log_types::ApplicationId(path.display().to_string()),
            store_id: store_id.clone(),
            is_official_example: false,
            started: re_log_types::Time::now(),
            store_source,
            store_kind: re_log_types::StoreKind::Recording,
        },
    }))
    .ok();
    // .ok(): we may be running in a background thread, so who knows if the receiver is still open

    // Send actual file.
    let log_msg = log_msg_from_file_path(store_id, path)?;
    tx.send(log_msg).ok();
    tx.quit(None).ok();
    Ok(())
}

fn log_msg_from_file_path(
    store_id: re_log_types::StoreId,
    file_path: &std::path::Path,
) -> anyhow::Result<LogMsg> {
    let entity_path = re_log_types::EntityPath::from_file_path_as_single_string(file_path);
    let cells = data_cells_from_file_path(file_path)?;

    let num_instances = cells.first().map_or(0, |cell| cell.num_instances());

    let timepoint = re_log_types::TimePoint::default();

    let data_row = re_log_types::DataRow::from_cells(
        re_log_types::RowId::new(),
        timepoint,
        entity_path,
        num_instances,
        cells,
    )?;

    let data_table =
        re_log_types::DataTable::from_rows(re_log_types::TableId::random(), [data_row]);
    let arrow_msg = data_table.to_arrow_msg()?;
    Ok(LogMsg::ArrowMsg(store_id, arrow_msg))
}

// Non-blocking
fn stream_rrd_file(
    path: std::path::PathBuf,
    tx: re_smart_channel::Sender<LogMsg>,
) -> anyhow::Result<()> {
    let version_policy = re_log_encoding::decoder::VersionPolicy::Warn;
    let file = std::fs::File::open(&path).context("Failed to open file")?;
    let decoder = re_log_encoding::decoder::Decoder::new(version_policy, file)?;

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
