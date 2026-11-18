use anyhow::Context as _;
use re_chunk::TimelineName;
use re_entity_db::EntityDb;
use re_log_msg::LogMsg;
use re_log_types::{ApplicationId, RecordingId, StoreKind};
use re_viewer_context::BlueprintUndoState;
#[cfg(not(target_arch = "wasm32"))]
use re_viewer_context::store_hub::{BlueprintPersistenceKey, CachedBlueprintSource};

/// A stable snapshot of the messages and file-format version needed to encode RRD data.
///
/// Keeping the snapshot separate from its encoding lets native saves write it in a background task
/// while web APIs can encode it into bytes for browser clients.
pub struct RrdSnapshot {
    pub version: re_build_info::CrateVersion<'static>,
    pub messages: Vec<re_chunk::ChunkResult<LogMsg>>,
}

impl RrdSnapshot {
    #[cfg(any(target_arch = "wasm32", test))]
    pub fn encode(self) -> anyhow::Result<Vec<u8>> {
        let Self { version, messages } = self;

        let mut bytes = Vec::new();
        re_log_encoding::Encoder::encode_into(
            version,
            re_log_encoding::rrd::EncodingOptions::PROTOBUF_COMPRESSED,
            messages,
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

/// A file name that is unique for `name`.
///
/// Converts to lowercase, replaces unsafe characters with `-` using
/// [`re_viewer_context::sanitize_file_name`], and limits the length. If that changes the name,
/// or the name has non-ASCII characters, or is empty or only dots, a hash of `name` is appended.
#[cfg(not(target_arch = "wasm32"))]
fn unique_file_name(name: &str) -> String {
    // The cached blueprints of an app are two directory levels below `blueprints`. Some versions
    // of Windows 10 limit paths to 260 characters.
    const MAX_NAME_LEN: usize = 64;

    let mut file_name = re_viewer_context::sanitize_file_name(&name.to_lowercase());
    file_name.truncate(file_name.floor_char_boundary(MAX_NAME_LEN));

    let only_dots = file_name.chars().all(|c| c == '.');

    // File systems that ignore case or Unicode normalization treat some distinct non-ASCII names
    // as the same name, like "σ" and "ς" on macOS.
    let has_non_ascii = !name.is_ascii();

    if file_name != name || has_non_ascii || only_dots {
        let hash = ahash::RandomState::with_seeds(1, 2, 3, 4).hash_one(name);
        file_name = format!("{file_name}-{hash:x}");
    }

    file_name
}

/// The directory with all persisted blueprints.
///
/// It is laid out like this:
///
/// ```text
/// blueprints/
/// ├── apps/
/// │   └── <app id>/
/// │       ├── @local.rbl    the last active blueprint that isn't from a dataset
/// │       └── <segment id>.rbl
/// ├── tables/
/// │   └── <table hash>.rbl
/// └── <app id>.rbl    one blueprint per app, saved by earlier versions
/// ```
#[cfg(not(target_arch = "wasm32"))]
fn blueprints_dir() -> anyhow::Result<std::path::PathBuf> {
    let Some(storage_dir) = eframe::storage_dir(crate::native::APP_ID) else {
        anyhow::bail!("Error finding project directory for blueprints.")
    };
    Ok(storage_dir.join("blueprints"))
}

/// The directory with the cached blueprints of an app.
#[cfg(not(target_arch = "wasm32"))]
fn app_blueprints_dir(app_id: &ApplicationId) -> anyhow::Result<std::path::PathBuf> {
    Ok(blueprints_dir()?
        .join("apps")
        .join(unique_file_name(app_id.as_str())))
}

/// Deletes the file at `path`, and returns whether it existed.
#[cfg(not(target_arch = "wasm32"))]
fn remove_file_if_exists(path: &std::path::Path, what: &str) -> anyhow::Result<bool> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(true),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(err) => Err(err).with_context(|| format!("Could not delete {what}: {path:?}")),
    }
}

/// Deletes the directory at `path` and everything in it, if it exists.
#[cfg(not(target_arch = "wasm32"))]
fn remove_dir_if_exists(path: &std::path::Path, what: &str) -> anyhow::Result<()> {
    match std::fs::remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err).with_context(|| format!("Could not delete {what}: {path:?}")),
    }
}

/// Determine the path of a persisted blueprint.
///
/// The same key always gives the same path, and different keys give different paths.
// TODO(#2579): Implement equivalent for web
#[cfg(not(target_arch = "wasm32"))]
pub fn blueprint_path(key: &BlueprintPersistenceKey) -> anyhow::Result<std::path::PathBuf> {
    match key {
        BlueprintPersistenceKey::Recording {
            application_id,
            source,
        } => recording_blueprint_path(application_id, source),
        BlueprintPersistenceKey::Table(key) => table_blueprint_path(key),
    }
}

/// Determine the path of the cached copy of one blueprint of an app.
#[cfg(not(target_arch = "wasm32"))]
fn recording_blueprint_path(
    app_id: &ApplicationId,
    source: &CachedBlueprintSource,
) -> anyhow::Result<std::path::PathBuf> {
    let app_dir = app_blueprints_dir(app_id)?;
    Ok(match source {
        CachedBlueprintSource::Local => app_dir.join("@local.rbl"),
        CachedBlueprintSource::Dataset(blueprint_id) => {
            app_dir.join(format!("{}.rbl", unique_file_name(blueprint_id.as_str())))
        }
    })
}

/// The path where earlier versions of the viewer saved the blueprint of an app.
#[cfg(not(target_arch = "wasm32"))]
fn legacy_recording_blueprint_path(app_id: &ApplicationId) -> anyhow::Result<std::path::PathBuf> {
    let blueprint_dir = blueprints_dir()?;

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

/// Load the blueprint of an app from its legacy path, where earlier versions of the viewer saved
/// one blueprint per app.
///
/// The file has the same format as any other blueprint file. It stays on disk until
/// [`delete_legacy_path_blueprint`] deletes it.
///
/// Returns `Ok(None)` if there is no such file, or if the app already has a directory of cached
/// blueprints, since those replace the file at the legacy path.
#[cfg(not(target_arch = "wasm32"))]
pub fn load_legacy_path_blueprint(
    app_id: &ApplicationId,
) -> anyhow::Result<Option<re_entity_db::StoreBundle>> {
    if app_blueprints_dir(app_id)?.exists() {
        return Ok(None);
    }
    let path = legacy_recording_blueprint_path(app_id)?;
    if !path.exists() {
        return Ok(None);
    }

    re_log::debug!("Loading blueprint saved by an earlier version from {path:?}");
    Ok(crate::loading::load_blueprint_file(&path))
}

/// Delete the blueprint of an app at its legacy path, if it exists.
#[cfg(not(target_arch = "wasm32"))]
pub fn delete_legacy_path_blueprint(app_id: &ApplicationId) -> anyhow::Result<()> {
    remove_file_if_exists(
        &legacy_recording_blueprint_path(app_id)?,
        "blueprint at legacy path",
    )?;
    Ok(())
}

/// Delete all cached blueprints of an app from disk.
#[cfg(not(target_arch = "wasm32"))]
pub fn clear_app_blueprints(app_id: &ApplicationId) -> anyhow::Result<()> {
    remove_dir_if_exists(&app_blueprints_dir(app_id)?, "app blueprints")
}

#[cfg(not(target_arch = "wasm32"))]
fn table_blueprints_dir() -> anyhow::Result<std::path::PathBuf> {
    Ok(blueprints_dir()?.join("tables"))
}

/// Determine the persisted blueprint path for a table.
#[cfg(not(target_arch = "wasm32"))]
fn table_blueprint_path(
    table_ref: &re_viewer_context::TableReference,
) -> anyhow::Result<std::path::PathBuf> {
    let key_hash = re_log_types::hash::Hash64::hash(table_ref).hash64();
    Ok(table_blueprints_dir()?.join(format!("{key_hash:016x}.rbl")))
}

/// Delete all persisted table blueprints from disk.
#[cfg(not(target_arch = "wasm32"))]
pub fn clear_table_blueprints() -> anyhow::Result<()> {
    remove_dir_if_exists(&table_blueprints_dir()?, "table blueprints")
}

/// Delete one persisted blueprint from disk, if it exists.
#[cfg(not(target_arch = "wasm32"))]
pub fn delete_blueprint(key: &BlueprintPersistenceKey) -> anyhow::Result<()> {
    let blueprint_path = blueprint_path(key)?;
    if remove_file_if_exists(&blueprint_path, "persisted blueprint")? {
        re_log::debug!("Deleted persisted blueprint at {blueprint_path:?}");
    }
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
pub fn encode_to_file(
    version: re_build_info::CrateVersion<'static>,
    path: &std::path::Path,
    messages: impl Iterator<Item = re_chunk::ChunkResult<re_log_msg::LogMsg>>,
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

    /// Names that are empty or only dots get a hash, so they never refer to the current or
    /// parent directory. Non-ASCII names get a hash too, so two names that a file system treats
    /// as the same name get different file names. A plain lowercase ASCII name stays as it is.
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn unique_file_name_special_names() {
        for name in ["", ".", ".."] {
            let file_name = unique_file_name(name);
            assert_ne!(file_name, name);
            assert!(!file_name.is_empty());
            assert!(!file_name.chars().all(|c| c == '.'), "{file_name:?}");
        }

        for (a, b) in [("σ", "ς"), ("ß", "ss"), ("é", "e\u{301}")] {
            assert_ne!(unique_file_name(a), a);
            assert_ne!(
                unique_file_name(a).to_lowercase(),
                unique_file_name(b).to_lowercase(),
                "{a:?} and {b:?}"
            );
        }

        assert_eq!(unique_file_name("my_app"), "my_app");
    }
}
