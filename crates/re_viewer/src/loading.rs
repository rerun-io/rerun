use crate::StoreBundle;

#[cfg(not(target_arch = "wasm32"))]
#[must_use]
pub fn load_file_path(path: &std::path::Path, with_notifications: bool) -> Option<StoreBundle> {
    fn load_file_path_impl(path: &std::path::Path) -> anyhow::Result<StoreBundle> {
        re_tracing::profile_function!();
        use anyhow::Context as _;
        let file = std::fs::File::open(path).context("Failed to open file")?;
        StoreBundle::from_rrd(file)
    }

    if with_notifications {
        re_log::info!("Loading {path:?}…");
    }

    match load_file_path_impl(path) {
        Ok(mut rrd) => {
            if with_notifications {
                re_log::info!("Loaded {path:?}");
            }
            for store_db in rrd.store_dbs_mut() {
                store_db.data_source =
                    Some(re_smart_channel::SmartChannelSource::File { path: path.into() });
            }
            Some(rrd)
        }
        Err(err) => {
            let msg = format!("Failed loading {path:?}: {}", re_error::format(&err));
            re_log::error!("{msg}");
            rfd::MessageDialog::new()
                .set_level(rfd::MessageLevel::Error)
                .set_description(&msg)
                .show();
            None
        }
    }
}
