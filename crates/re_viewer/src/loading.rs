use crate::{store_hub::StoreLoadError, StoreBundle};

#[derive(thiserror::Error, Debug)]
enum BlueprintLoadError {
    #[error("Failed to open file: {0}")]
    FileOpen(#[from] std::io::Error),

    #[error(transparent)]
    StoreLoad(#[from] StoreLoadError),
}

/// Try to load the given `.blueprint` file.
///
/// The file must be of a matching version of rerun.
#[must_use]
pub fn load_blueprint_file(
    path: &std::path::Path,
    with_notifications: bool,
) -> Option<crate::StoreBundle> {
    fn load_file_path_impl(path: &std::path::Path) -> Result<StoreBundle, BlueprintLoadError> {
        re_tracing::profile_function!();

        let file = std::fs::File::open(path)?;

        // Blueprint files change often. Be strict about the version, and then ignore any errors.
        // See https://github.com/rerun-io/rerun/issues/2830
        let version_policy = re_log_encoding::decoder::VersionPolicy::Error;

        Ok(StoreBundle::from_rrd(version_policy, file)?)
    }

    match load_file_path_impl(path) {
        Ok(mut rrd) => {
            if with_notifications {
                re_log::info!("Loaded {path:?}");
            }

            for entity_db in rrd.entity_dbs_mut() {
                entity_db.data_source =
                    Some(re_smart_channel::SmartChannelSource::File(path.into()));
            }
            Some(rrd)
        }
        Err(err) => {
            let msg = format!("Failed loading {path:?}: {err}");

            if with_notifications {
                re_log::error!("{msg}");
                rfd::MessageDialog::new()
                    .set_level(rfd::MessageLevel::Error)
                    .set_description(&msg)
                    .show();
            } else {
                // Silently ignore
                re_log::debug!("{msg}");
            }
            None
        }
    }
}

use re_log_types::LogMsg;

pub fn blueprint_rewriter(
    app_id: re_log_types::ApplicationId,
    blueprint_id: re_log_types::StoreId,
    rx: re_smart_channel::Receiver<LogMsg>,
) -> re_smart_channel::Receiver<LogMsg> {
    let (new_tx, new_rx) = rx.chained_channel();

    std::thread::Builder::new()
        .name("blueprint_id_rewriter".to_owned())
        .spawn(move || {
            while let Ok(mut msg) = rx.recv_with_send_time() {
                let payload = match msg.payload {
                    re_smart_channel::SmartMessagePayload::Msg(msg) => msg,
                    re_smart_channel::SmartMessagePayload::Quit(err) => {
                        if let Some(err) = err {
                            re_log::warn!(
                                "Data source {} has left unexpectedly: {err}",
                                msg.source
                            );
                        } else {
                            re_log::debug!("Data source {} has left", msg.source);
                        }
                        continue;
                    }
                };

                let patched_payload = match payload {
                    LogMsg::SetStoreInfo(store_info) => {
                        let mut info = store_info.info;
                        info.application_id = app_id.clone();
                        info.store_id = blueprint_id.clone();
                        LogMsg::SetStoreInfo(re_log_types::SetStoreInfo { info, ..store_info })
                    }
                    LogMsg::ArrowMsg(_, arrow_msg) => {
                        // TODO(jleibs): Patching timestamps is a major pain.
                        // not sure what the right thing to do here is.
                        LogMsg::ArrowMsg(blueprint_id.clone(), arrow_msg)
                    }
                };

                msg.payload = re_smart_channel::SmartMessagePayload::Msg(patched_payload);

                new_tx.send_at(msg.time, msg.source, msg.payload).ok();
            }
        })
        .unwrap();

    new_rx
}
