//! Snapshot tests for the mp4 importer.
//!
//! Captures the chunk schema (`ChunkColumnDescriptors`) produced by importing an
//! mp4 fixture. The snapshot pinpoints regressions in entity paths, timeline
//! names/types, component descriptors, and sortedness flags.
//!
//! This is shape-coverage, not byte-coverage: it does *not* verify that the
//! `AssetVideo` blob bytes themselves are identical across runs. The blob is
//! the file's `contents` passed verbatim into `AssetVideo::new(contents)`, so
//! byte-equality follows by inspection of the call path; the snapshot guards
//! everything around the blob.

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use re_chunk::Chunk;
    use re_chunk_store::{ChunkStore, ChunkStoreConfig, ChunkStoreHandle};
    use re_importer::{ArchetypeImporter, ImportedData, Importer as _, ImporterSettings};
    use re_log_types::StoreId;

    /// Resolve a path under the workspace-root `tests/assets/video/` directory.
    ///
    /// Mirrors the pattern at `crates/utils/re_video/src/encode/ffmpeg_cli.rs`
    /// where `env!("CARGO_MANIFEST_DIR").ancestors().nth(3)` walks
    /// `crates/store/re_importer → crates/store → crates → repo-root`.
    fn fixture(name: &str) -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(3)
            .expect("workspace root is three ancestors up from crates/store/re_importer")
            .join("tests/assets/video")
            .join(name)
    }

    /// Load an mp4 file through the `ArchetypeImporter` and collect every emitted chunk.
    fn load_video_chunks(path: impl AsRef<std::path::Path>) -> Vec<Chunk> {
        let path = path.as_ref().to_path_buf();
        println!("Loading MP4 file: {}", path.display());
        let (tx, rx) = crossbeam::channel::bounded(1024);
        let settings = ImporterSettings::recommended("test");
        ArchetypeImporter
            .import_from_path(&settings, path, tx.clone())
            .unwrap();
        drop(tx);
        rx.iter().filter_map(ImportedData::into_chunk).collect()
    }

    /// Fail-fast vs. lenient is the *importer's* policy, not the reader crate's.
    /// `re_mp4_reader` is a lenient producer (it emits the asset chunk and skips
    /// only the frame-reference index when frame timestamps are unreadable); the
    /// importer is what decides whether a bad video aborts the import.
    ///
    /// This pins the pre-extraction behavior: a non-mp4 blob with a `.mp4`
    /// extension still imports successfully, logging the `AssetVideo` but no
    /// `VideoFrameReference` index. (A genuine chunk-construction error would be
    /// fatal — see `load_video` — but that path isn't reachable from input bytes.)
    #[test]
    fn test_mp4_importer_unreadable_video_logs_asset_only() {
        use re_sdk_types::archetypes::{AssetVideo, VideoFrameReference};

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("corrupt.mp4");
        std::fs::write(&path, b"not a real mp4").unwrap();

        // `load_video_chunks` unwraps the import result, so this also asserts the
        // import does not fail.
        let chunks = load_video_chunks(&path);

        let has = |descriptor| {
            chunks
                .iter()
                .any(|c| c.component_descriptors().any(|d| *d == descriptor))
        };
        assert!(
            has(AssetVideo::descriptor_blob()),
            "the video asset should still be logged"
        );
        assert!(
            !has(VideoFrameReference::descriptor_timestamp()),
            "no frame-reference index should be produced for an unreadable video"
        );
    }

    #[test]
    fn test_mp4_importer_h264_nobframes() {
        let chunks = load_video_chunks(fixture("Big_Buck_Bunny_1080_1s_h264_nobframes.mp4"));

        let store = ChunkStore::new(
            StoreId::random(re_log_types::StoreKind::Recording, "test_mp4_importer"),
            ChunkStoreConfig::default(),
        );
        let store_handle = ChunkStoreHandle::new(store);

        {
            let mut store = store_handle.write();
            for chunk in chunks {
                store.insert_chunk(&Arc::new(chunk)).unwrap();
            }
        }

        let schema = store_handle.read().schema().chunk_column_descriptors();

        // The importer derives entity paths from the full filesystem path of the
        // fixture (`EntityPath::from_file_path`). Strip the absolute prefix up to
        // and including `/tests/assets/video/` so the snapshot is portable across
        // machines and CI checkouts.
        insta::with_settings!({
            filters => vec![(r"/[^\s]*/tests/assets/video/", "/tests/assets/video/")],
        }, {
            insta::assert_debug_snapshot!("h264_nobframes", schema);
        });
    }
}
