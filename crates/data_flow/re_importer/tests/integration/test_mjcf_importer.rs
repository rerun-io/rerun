//! End-to-end test of the MJCF importer.

#[cfg(test)]
mod tests {
    use re_chunk::Chunk;
    use re_importer::{ImportedData, Importer as _, ImporterSettings, MjcfImporter};
    use re_sdk_types::archetypes::{Asset3D, Transform3D};

    fn import_chunks(path: &str) -> Result<Vec<Chunk>, re_importer::ImporterError> {
        let (tx, rx) = crossbeam::channel::bounded(1024);
        let settings = ImporterSettings::recommended("test");
        MjcfImporter.import_from_path(&settings, path.into(), tx.clone())?;
        drop(tx);
        Ok(rx.iter().filter_map(ImportedData::into_chunk).collect())
    }

    /// Includes are resolved, defaults applied, and a mesh that cannot be loaded does not abort
    /// the import of everything else.
    #[test]
    fn test_mjcf_importer_scene() {
        let chunks = import_chunks("tests/assets/mjcf/scene.xml").unwrap();

        let mut entity_paths = chunks
            .iter()
            .map(|chunk| chunk.entity_path().to_string())
            .collect::<Vec<_>>();
        entity_paths.sort();
        entity_paths.dedup();
        insta::assert_debug_snapshot!("mjcf_scene", entity_paths);

        let chunks_with = |component| {
            chunks
                .iter()
                .filter(move |chunk| chunk.components().contains_component(component))
        };
        assert_eq!(
            chunks_with(Asset3D::descriptor_blob().component).count(),
            1,
            "only the tetrahedron mesh can be loaded"
        );
        assert_eq!(
            chunks_with(Transform3D::descriptor_child_frame().component)
                .map(Chunk::num_rows)
                .sum::<usize>(),
            3,
            "one transform per body, excluding the world"
        );
    }

    #[test]
    fn test_mjcf_importer_ignores_other_xml() {
        let err = import_chunks("tests/assets/mjcf/not_mjcf.xml").unwrap_err();
        assert!(err.is_incompatible(), "{err}");
    }
}
