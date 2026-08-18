use anyhow::Context as _;
use re_chunk::TimelineName;
use re_entity_db::EntityDb;
use re_log_types::{ApplicationId, LogMsg, RecordingId, StoreKind};
use re_viewer_context::BlueprintUndoState;

/// A stable snapshot of the messages and file-format version needed to encode RRD data.
///
/// Keeping the snapshot separate from its encoding lets native saves write it in a background task
/// while web APIs can encode it into bytes for browser clients.
pub struct RrdSnapshot {
    pub version: re_build_info::CrateVersion,
    pub messages: Vec<re_chunk::ChunkResult<LogMsg>>,
}

impl RrdSnapshot {
    #[cfg(any(target_arch = "wasm32", test))]
    pub fn encode(self) -> anyhow::Result<Vec<u8>> {
        let mut bytes = Vec::new();
        re_log_encoding::Encoder::encode_into(
            self.version,
            re_log_encoding::rrd::EncodingOptions::PROTOBUF_COMPRESSED,
            self.messages,
            &mut bytes,
        )
        .context("Encoding RRD snapshot")?;

        Ok(bytes)
    }

    pub fn recording(
        entity_db: &EntityDb,
        time_selection: Option<(TimelineName, re_log_types::AbsoluteTimeRangeF)>,
    ) -> Self {
        re_tracing::profile_function!();

        re_log::debug_assert_eq!(entity_db.store_kind(), StoreKind::Recording);

        let version = entity_db
            .store_info()
            .and_then(|info| info.store_version)
            .unwrap_or(re_build_info::CrateVersion::LOCAL);

        Self {
            version,
            messages: entity_db.to_messages(time_selection).collect(),
        }
    }

    pub fn blueprint(
        blueprint: &EntityDb,
        undo_state: Option<&BlueprintUndoState>,
    ) -> anyhow::Result<Self> {
        re_tracing::profile_function!();

        re_log::debug_assert_eq!(blueprint.store_kind(), StoreKind::Blueprint);

        let version = blueprint
            .store_info()
            .and_then(|info| info.store_version)
            .unwrap_or(re_build_info::CrateVersion::LOCAL);

        // A saved blueprint must not merge with its source when it is loaded again.
        let new_store_id = blueprint
            .store_id()
            .clone()
            .with_recording_id(RecordingId::random());
        let mut saved_blueprint = blueprint
            .clone_with_new_id(new_store_id)
            .context("Cloning current blueprint")?;

        if let Some(undo_state) = undo_state {
            // Preserve live undo state while omitting redo entries from the saved copy.
            undo_state.clone().clear_redo_buffer(&mut saved_blueprint);
        }

        Ok(Self {
            version,
            messages: saved_blueprint.to_messages(None).collect(),
        })
    }
}

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

/// Delete the persisted blueprint for the given `ApplicationId` from disk, if it exists.
#[cfg(not(target_arch = "wasm32"))]
pub fn delete_blueprint(app_id: &ApplicationId) -> anyhow::Result<()> {
    let blueprint_path = default_blueprint_path(app_id)?;
    if blueprint_path.exists() {
        std::fs::remove_file(&blueprint_path)?;
        re_log::debug!("Deleted persisted blueprint for {app_id} at {blueprint_path:?}");
    }
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
pub fn encode_to_file(
    version: re_build_info::CrateVersion,
    path: &std::path::Path,
    messages: impl Iterator<Item = re_chunk::ChunkResult<re_log_types::LogMsg>>,
) -> anyhow::Result<()> {
    re_tracing::profile_function!();

    let mut file = std::fs::File::create(path)
        .with_context(|| format!("Failed to create file at {path:?}"))?;

    let encoding_options = re_log_encoding::rrd::EncodingOptions::PROTOBUF_COMPRESSED;
    re_log_encoding::Encoder::encode_into(version, encoding_options, messages, &mut file)
        .map(|_| ())
        .context("Message encode")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recording_snapshot_decodes_as_rrd() {
        use re_chunk::{RowId, TimePoint};

        let mut test_context = re_test_context::TestContext::new();
        test_context.log_entity("test", |builder| {
            builder.with_archetype(
                RowId::new(),
                TimePoint::STATIC,
                &re_sdk_types::archetypes::Scalars::single(42.0),
            )
        });

        let bytes = {
            let store_hub = test_context.store_hub.lock();
            let recording = store_hub
                .entity_db(&test_context.recording_store_id)
                .expect("test recording should exist");
            RrdSnapshot::recording(recording, None)
                .encode()
                .expect("recording should encode")
        };

        let bundle = re_entity_db::StoreBundle::from_rrd(
            std::io::BufReader::new(std::io::Cursor::new(bytes)),
            &re_log_channel::LogSource::Sdk,
        )
        .expect("saved recording should decode");

        let recordings = bundle.recordings().collect::<Vec<_>>();
        assert_eq!(recordings.len(), 1);
        assert!(recordings[0].format_with_components().contains("test"));
    }
}
