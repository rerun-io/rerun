//! Tests for importing audio files as `AssetAudio`.

use re_chunk::Chunk;
use re_importer::{ArchetypeImporter, ImportedData, Importer as _, ImporterSettings};
use re_log_types::{TimeInt, Timeline};
use re_sdk_types::FromArrow as _;
use re_sdk_types::archetypes::AssetAudio;
use re_sdk_types::components::MediaType;

fn fixture(name: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .expect("workspace root is three ancestors up from crates/data_flow/re_importer")
        .join("tests/assets/audio")
        .join(name)
}

fn load_chunks(path: impl AsRef<std::path::Path>) -> Vec<Chunk> {
    let (tx, rx) = crossbeam::channel::bounded(1024);
    let settings = ImporterSettings::recommended("test");
    ArchetypeImporter
        .import_from_path(&settings, path.as_ref().to_path_buf(), tx.clone())
        .expect("import should succeed");
    drop(tx);
    rx.iter().filter_map(ImportedData::into_chunk).collect()
}

fn media_type_of(chunk: &Chunk) -> MediaType {
    let column = chunk
        .components()
        .get(AssetAudio::descriptor_media_type().component)
        .expect("media type column");
    let values = MediaType::from_arrow(column.list_array.values().as_ref())
        .expect("media type should deserialize");
    values
        .into_iter()
        .next()
        .expect("media type column should have one value")
}

#[test]
fn test_audio_importer_wav_and_aac() {
    for (file, expected_media_type) in [
        ("sine_440hz_2s.wav", MediaType::wav()),
        ("sine_440hz_2s.aac", MediaType::aac()),
    ] {
        let chunks = load_chunks(fixture(file));
        assert_eq!(chunks.len(), 1, "{file}");
        let chunk = &chunks[0];

        assert_eq!(chunk.num_rows(), 1, "{file}");
        assert!(
            chunk
                .component_descriptors()
                .any(|d| *d == AssetAudio::descriptor_blob()),
            "{file}: missing blob"
        );
        assert_eq!(media_type_of(chunk), expected_media_type, "{file}");

        let audio_timeline = chunk
            .timelines()
            .get(Timeline::new_duration("audio").name())
            .expect("audio timeline");
        assert_eq!(
            audio_timeline.times().next(),
            Some(TimeInt::ZERO),
            "{file}: audio should start at zero"
        );
    }
}
