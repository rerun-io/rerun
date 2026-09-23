//! End-to-end test of the URDF importer.

#[cfg(test)]
mod tests {
    use re_chunk::Chunk;
    use re_importer::{ImportedData, Importer as _, ImporterSettings, UrdfImporter};

    fn import_chunks(path: impl AsRef<std::path::Path>) -> Vec<Chunk> {
        let path = path.as_ref().to_path_buf();
        let (tx, rx) = crossbeam::channel::bounded(1024);
        let settings = ImporterSettings::recommended("test");
        UrdfImporter
            .import_from_path(&settings, path, tx.clone())
            .unwrap();
        drop(tx);
        rx.iter().filter_map(ImportedData::into_chunk).collect()
    }

    /// A mesh that cannot be loaded must not abort the import: every other link,
    /// joint and geometry still has to make it through.
    #[test]
    fn test_urdf_importer_dangling_mesh() {
        let chunks = import_chunks("tests/assets/urdf/dangling_mesh.urdf");

        let mut entity_paths = chunks
            .iter()
            .map(|chunk| chunk.entity_path().to_string())
            .collect::<Vec<_>>();
        entity_paths.sort();
        entity_paths.dedup();

        insta::assert_debug_snapshot!("dangling_mesh", entity_paths);

        assert!(
            !chunks.iter().any(|chunk| {
                chunk.components().contains_component(
                    re_sdk_types::archetypes::Asset3D::descriptor_blob().component,
                )
            }),
            "the only mesh in the file is unresolvable, so no Asset3D should have been emitted"
        );
    }
}
