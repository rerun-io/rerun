use re_log_types::ApplicationId;

/// Convert to lowercase and replace any character that is not a fairly common
/// filename character with '-'
pub fn sanitize_app_id(app_id: &ApplicationId) -> String {
    re_viewer_context::sanitize_file_name(&app_id.as_str().to_lowercase())
}

/// Determine the default path for a blueprint based on its `ApplicationId`
/// This path should be deterministic and unique.
// TODO(#2579): Implement equivalent for web
#[cfg(not(target_arch = "wasm32"))]
pub fn default_blueprint_path(app_id: &ApplicationId) -> anyhow::Result<std::path::PathBuf> {
    use anyhow::Context as _;

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
    let extension_part_length = ".rbl".len();
    let total_reserved_length = directory_part_length + hash_part_length + extension_part_length;
    if total_reserved_length > MAX_PATH {
        anyhow::bail!(
            "Could not form blueprint path: total minimum length exceeds {MAX_PATH} characters."
        )
    }
    sanitized_app_id.truncate(MAX_PATH - total_reserved_length);

    // If the sanitization actually did something, we no longer have a uniqueness guarantee,
    // so insert the hash.
    if sanitized_app_id != app_id.as_str() {
        // Hash the original app-id.

        let hash = ahash::RandomState::with_seeds(1, 2, 3, 4).hash_one(app_id.as_str());

        sanitized_app_id = format!("{sanitized_app_id}-{hash:x}");
    }

    Ok(blueprint_dir.join(format!("{sanitized_app_id}.rbl")))
}

#[cfg(not(target_arch = "wasm32"))]
pub fn encode_to_file(
    version: re_build_info::CrateVersion,
    path: &std::path::Path,
    messages: impl Iterator<Item = re_chunk::ChunkResult<re_log_types::LogMsg>>,
) -> anyhow::Result<()> {
    re_tracing::profile_function!();
    use anyhow::Context as _;

    let mut file = std::fs::File::create(path)
        .with_context(|| format!("Failed to create file at {path:?}"))?;

    let encoding_options = re_log_encoding::rrd::EncodingOptions::PROTOBUF_COMPRESSED;
    re_log_encoding::Encoder::encode_into(version, encoding_options, messages, &mut file)
        .map(|_| ())
        .context("Message encode")
}
