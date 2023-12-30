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
/// This path should be deterministic and unique.
// TODO(#2579): Implement equivalent for web
pub fn default_blueprint_path(app_id: &ApplicationId) -> anyhow::Result<std::path::PathBuf> {
    use anyhow::Context;

    let Some(storage_dir) = eframe::storage_dir(crate::native::APP_ID) else {
        anyhow::bail!("Error finding project directory for blueprints.")
    };

    let blueprint_dir = storage_dir.join("blueprints");
    std::fs::create_dir_all(&blueprint_dir)
        .context("Could not create blueprint save directory.")?;

    // We want a unique filename (not a directory) for each app-id.

    // First we sanitize to remove disallowed characters
    let mut sanitized_app_id = sanitize_app_id(app_id);

    // Make sure the filename isn't too long
    // This is overly conservative in most cases but some versions of Windows 10
    // still have this restriction.
    // TODO(jleibs): Determine this value from the environment.
    const MAX_PATH: usize = 255;
    let directory_part_length = blueprint_dir.as_os_str().len();
    let hash_part_length = 16 + 1;
    let extension_part_length = ".blueprint".len();
    let total_reserved_length = directory_part_length + hash_part_length + extension_part_length;
    if total_reserved_length > MAX_PATH {
        anyhow::bail!(
            "Could not form blueprint path: total minimum length exceeds {MAX_PATH} characters."
        )
    }
    sanitized_app_id.truncate(MAX_PATH - total_reserved_length);

    // If the sanitization actually did something, we no longer have a uniqueness guarantee,
    // so insert the hash.
    if sanitized_app_id != app_id.0 {
        // Hash the original app-id.

        let hash = ahash::RandomState::with_seeds(1, 2, 3, 4).hash_one(&app_id.0);

        sanitized_app_id = format!("{sanitized_app_id}-{hash:x}");
    }

    Ok(blueprint_dir.join(format!("{sanitized_app_id}.blueprint")))
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
    use re_arrow_store::TimeRange;

    re_tracing::profile_function!();

    store_db.store().sort_indices_if_needed();

    let set_store_info_msg = store_db
        .store_info_msg()
        .map(|msg| LogMsg::SetStoreInfo(msg.clone()));

    let time_filter = time_selection.map(|(timeline, range)| {
        (
            timeline,
            TimeRange::new(range.min.floor(), range.max.ceil()),
        )
    });
    let data_msgs: Result<Vec<_>, _> = store_db
        .store()
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

    let msgs = std::iter::once(set_store_info_msg)
        .flatten() // option
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
