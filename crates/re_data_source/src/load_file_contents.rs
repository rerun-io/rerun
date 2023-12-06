use re_log_encoding::decoder::VersionPolicy;
use re_log_types::{FileSource, LogMsg};
use re_smart_channel::Sender;

use crate::{load_file::data_cells_from_file_contents, FileContents};

#[allow(clippy::needless_pass_by_value)] // false positive on some feature flags
pub fn load_file_contents(
    store_id: re_log_types::StoreId,
    file_source: FileSource,
    file_contents: FileContents,
    tx: Sender<LogMsg>,
) -> anyhow::Result<()> {
    let file_name = file_contents.name.clone();
    re_tracing::profile_function!(file_name.as_str());
    re_log::info!("Loading {file_name:?}…");

    if file_name.ends_with(".rrd") {
        if cfg!(target_arch = "wasm32") {
            load_rrd_sync(&file_contents, &tx)
        } else {
            // Load in background thread on native:
            rayon::spawn(move || {
                if let Err(err) = load_rrd_sync(&file_contents, &tx) {
                    re_log::error!("Failed to load {file_name:?}: {err}");
                }
            });
            Ok(())
        }
    } else {
        // non-rrd = image or mesh:
        if cfg!(target_arch = "wasm32") {
            load_and_send(store_id, file_source, file_contents, &tx)
        } else {
            rayon::spawn(move || {
                let name = file_contents.name.clone();
                if let Err(err) = load_and_send(store_id, file_source, file_contents, &tx) {
                    re_log::error!("Failed to load {name:?}: {err}");
                }
            });
            Ok(())
        }
    }
}

fn load_and_send(
    store_id: re_log_types::StoreId,
    file_source: FileSource,
    file_contents: FileContents,
    tx: &Sender<LogMsg>,
) -> anyhow::Result<()> {
    use re_log_types::SetStoreInfo;

    re_tracing::profile_function!(file_contents.name.as_str());

    // First, set a store info since this is the first thing the application expects.
    tx.send(LogMsg::SetStoreInfo(SetStoreInfo {
        row_id: re_log_types::RowId::new(),
        info: re_log_types::StoreInfo {
            application_id: re_log_types::ApplicationId(file_contents.name.clone()),
            store_id: store_id.clone(),
            is_official_example: false,
            started: re_log_types::Time::now(),
            store_source: re_log_types::StoreSource::File { file_source },
            store_kind: re_log_types::StoreKind::Recording,
        },
    }))
    .ok();
    // .ok(): we may be running in a background thread, so who knows if the receiver is still open

    // Send actual file.
    let log_msg = log_msg_from_file_contents(store_id, file_contents)?;
    tx.send(log_msg).ok();
    tx.quit(None).ok();
    Ok(())
}

fn log_msg_from_file_contents(
    store_id: re_log_types::StoreId,
    file_contents: FileContents,
) -> anyhow::Result<LogMsg> {
    let FileContents { name, bytes } = file_contents;

    let entity_path = re_log_types::EntityPath::from_single_string(name.clone());
    let cells = data_cells_from_file_contents(&name, bytes.to_vec())?;

    let num_instances = cells.first().map_or(0, |cell| cell.num_instances());

    let timepoint = re_log_types::TimePoint::default();

    let data_row = re_log_types::DataRow::from_cells(
        re_log_types::RowId::new(),
        timepoint,
        entity_path,
        num_instances,
        cells,
    )?;

    let data_table = re_log_types::DataTable::from_rows(re_log_types::TableId::new(), [data_row]);
    let arrow_msg = data_table.to_arrow_msg()?;
    Ok(LogMsg::ArrowMsg(store_id, arrow_msg))
}

fn load_rrd_sync(file_contents: &FileContents, tx: &Sender<LogMsg>) -> anyhow::Result<()> {
    re_tracing::profile_function!(file_contents.name.as_str());

    let bytes: &[u8] = &file_contents.bytes;
    let decoder = re_log_encoding::decoder::Decoder::new(VersionPolicy::Warn, bytes)?;
    for msg in decoder {
        tx.send(msg?)?;
    }
    re_log::debug!("Finished loading {:?}.", file_contents.name);
    Ok(())
}
