use re_log_types::LogMsg;
use re_smart_channel::Sender;

use crate::FileContents;

#[allow(clippy::needless_pass_by_value)] // false positive on some feature flags
pub fn load_file_contents(
    _store_id: re_log_types::StoreId,
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
        // TODO(emilk): support loading images and meshes from file contents
        anyhow::bail!("Unsupported file extension for {file_name:?}.");
    }
}

fn load_rrd_sync(file_contents: &FileContents, tx: &Sender<LogMsg>) -> Result<(), anyhow::Error> {
    let bytes: &[u8] = &file_contents.bytes;
    let decoder = re_log_encoding::decoder::Decoder::new(bytes)?;
    for msg in decoder {
        tx.send(msg?)?;
    }
    re_log::debug!("Finished loading {:?}.", file_contents.name);
    Ok(())
}
