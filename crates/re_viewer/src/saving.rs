#[cfg(not(target_arch = "wasm32"))]
use re_data_store::StoreDb;

#[cfg(not(target_arch = "wasm32"))]
use re_log_types::ApplicationId;

#[cfg(not(target_arch = "wasm32"))]
/// Determine the default path for a blueprint based on its `ApplicationId`
/// This path should be determnistic and unique.
// TODO(2579): Implement equivalent for web
pub fn default_blueprint_path(app_id: &ApplicationId) -> anyhow::Result<std::path::PathBuf> {
    use std::hash::{BuildHasher, Hash as _, Hasher as _};

    use anyhow::Context;

    // TODO(jleibs) is there a better way to get this folder from egui?
    if let Some(proj_dirs) = directories_next::ProjectDirs::from("", "", "rerun") {
        let data_dir = proj_dirs.data_dir().join("blueprints");
        if let Err(err) = std::fs::create_dir_all(&data_dir) {
            re_log::warn!(
                "Saving blueprints disabled: Failed to create blueprint directory at {:?}: {}",
                data_dir,
                err
            );
            Err(err).context("Could not create blueprint save directory.")
        } else {
            // We want a unique filename (not a directory) for each app-id.

            // First we sanitize to remove disallowed characters
            // TODO(jleibs): Maybe we should just restrict app-ids to valid filename characters.
            let options = sanitize_filename::Options {
                truncate: true,
                windows: true,
                replacement: "-",
            };

            let mut sanitized_app_id = sanitize_filename::sanitize_with_options(&app_id.0, options);

            // Make sure we are leaving space for the hash and file extension
            const MAX_LENGTH: usize = 255 - 16 - 1 - ".blueprint".len();
            sanitized_app_id.truncate(MAX_LENGTH);

            // If the sanitization actually did something, we no longer have a uniqueness guarantee,
            // so insert the hash.
            if sanitized_app_id != app_id.0 {
                // Hash the original app-id.
                let salt: u64 = 0xc927_d8cd_910d_16a3;

                let hash = {
                    let mut hasher = ahash::RandomState::with_seeds(1, 2, 3, 4).build_hasher();
                    salt.hash(&mut hasher);
                    app_id.0.hash(&mut hasher);
                    hasher.finish()
                };

                sanitized_app_id = format!("{sanitized_app_id}-{hash:x}");
            }

            Ok(data_dir.join(format!("{sanitized_app_id}.blueprint")))
        }
    } else {
        anyhow::bail!("Error finding project directory for blueprints.")
    }
}

#[cfg(not(target_arch = "wasm32"))]
/// Returns a closure that, when run, will save the contents of the current database
/// to disk, at the specified `path`.
///
/// If `time_selection` is specified, then only data for that specific timeline over that
/// specific time range will be accounted for.
pub fn save_database_to_file(
    store_db: &StoreDb,
    path: std::path::PathBuf,
    time_selection: Option<(re_data_store::Timeline, re_log_types::TimeRangeF)>,
) -> anyhow::Result<impl FnOnce() -> anyhow::Result<std::path::PathBuf>> {
    use itertools::Itertools as _;
    use re_arrow_store::TimeRange;

    re_tracing::profile_scope!("dump_messages");

    let begin_rec_msg = store_db
        .recording_msg()
        .map(|msg| LogMsg::SetStoreInfo(msg.clone()));

    let ent_op_msgs = store_db
        .iter_entity_op_msgs()
        .map(|msg| LogMsg::EntityPathOpMsg(store_db.store_id().clone(), msg.clone()))
        .collect_vec();

    let time_filter = time_selection.map(|(timeline, range)| {
        (
            timeline,
            TimeRange::new(range.min.floor(), range.max.ceil()),
        )
    });
    let data_msgs: Result<Vec<_>, _> = store_db
        .entity_db
        .data_store
        .to_data_tables(time_filter)
        .map(|table| {
            table
                .to_arrow_msg()
                .map(|msg| LogMsg::ArrowMsg(store_db.store_id().clone(), msg))
        })
        .collect();

    use anyhow::Context as _;
    use re_log_types::LogMsg;
    let data_msgs = data_msgs.with_context(|| "Failed to export to data tables")?;

    let msgs = std::iter::once(begin_rec_msg)
        .flatten() // option
        .chain(ent_op_msgs)
        .chain(data_msgs);

    Ok(move || {
        re_tracing::profile_scope!("save_to_file");

        use anyhow::Context as _;
        let file = std::fs::File::create(path.as_path())
            .with_context(|| format!("Failed to create file at {path:?}"))?;

        let encoding_options = re_log_encoding::EncodingOptions::COMPRESSED;
        re_log_encoding::encoder::encode_owned(encoding_options, msgs, file)
            .map(|_| path)
            .context("Message encode")
    })
}
