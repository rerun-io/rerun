#[cfg(not(target_arch = "wasm32"))]
use re_data_store::StoreDb;

#[cfg(not(target_arch = "wasm32"))]
use re_log_types::ApplicationId;

#[cfg(not(target_arch = "wasm32"))]
/// Convert to lowercase and replace any character that is not a fairly common
/// filename character with '-'
fn sanitize_app_id(app_id: &ApplicationId) -> String {
    let output = app_id.0.to_lowercase();
    output.replace(
        |c: char| !matches!(c, '0'..='9' | 'a'..='z' | '.' | '_' | '+' | '(' | ')' | '[' | ']'),
        "-",
    )
}

#[cfg(not(target_arch = "wasm32"))]
/// Determine the default path for a blueprint based on its `ApplicationId`
/// This path should be determnistic and unique.
// TODO(#2579): Implement equivalent for web
pub fn default_blueprint_path(app_id: &ApplicationId) -> anyhow::Result<std::path::PathBuf> {
    use std::hash::{BuildHasher, Hash as _, Hasher as _};

    use anyhow::Context;

    if let Some(data_dir) = eframe::storage_dir(crate::native::APP_ID) {
        std::fs::create_dir_all(&data_dir).context("Could not create blueprint save directory.")?;

        // We want a unique filename (not a directory) for each app-id.

        // First we sanitize to remove disallowed characters
        let mut sanitized_app_id = sanitize_app_id(app_id);

        // Make sure the filename isn't too long
        // This is overly conservative in most cases but some versions of Windows 10
        // still have this restriction.
        // TODO(jleibs): Determine this value from the environment.
        const MAX_PATH: usize = 255;
        let directory_part_length = data_dir.as_os_str().len();
        let hash_part_length = 16 + 1;
        let extension_part_length = ".blueprint".len();
        let total_reserved_length =
            directory_part_length + hash_part_length + extension_part_length;
        if total_reserved_length > MAX_PATH {
            anyhow::bail!("Could not form blueprint path: total minimum length exceeds {MAX_PATH} characters.")
        }
        sanitized_app_id.truncate(MAX_PATH - total_reserved_length);

        // If the sanitization actually did something, we no longer have a uniqueness guarantee,
        // so insert the hash.
        if sanitized_app_id != app_id.0 {
            // Hash the original app-id.

            let hash = {
                let mut hasher = ahash::RandomState::with_seeds(1, 2, 3, 4).build_hasher();
                app_id.0.hash(&mut hasher);
                hasher.finish()
            };

            sanitized_app_id = format!("{sanitized_app_id}-{hash:x}");
        }

        Ok(data_dir.join(format!("{sanitized_app_id}.blueprint")))
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

    re_tracing::profile_function!();

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
