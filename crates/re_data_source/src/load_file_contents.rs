use re_log_types::LogMsg;
use re_smart_channel::Sender;

use crate::FileContents;

#[allow(clippy::needless_pass_by_value)] // false positive on some feature flags
pub fn load_file_contents(
    _store_id: re_log_types::StoreId,
    file_contents: &FileContents,
    tx: Sender<LogMsg>,
) -> anyhow::Result<()> {
    let file_name = &file_contents.name;
    re_tracing::profile_function!(file_name);
    re_log::info!("Loading {file_name:?}…");

    if file_name.ends_with(".rrd") {
        // TODO: background thread on native
        let bytes: &[u8] = &file_contents.bytes;
        let decoder = re_log_encoding::decoder::Decoder::new(bytes)?;
        for msg in decoder {
            tx.send(msg?)?;
        }
        re_log::debug!("Finished loading {file_name:?}.");
        Ok(())
    } else {
        // TODO: support images and meshes
        anyhow::bail!("Unsupported file extension for {file_name:?}.");
    }
}
