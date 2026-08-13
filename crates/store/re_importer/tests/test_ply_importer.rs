//! End-to-end test of the `.ply` importer: file on disk → `ArchetypeImporter` → chunks.
//!
//! `cactus.ply` is a real 3D Gaussian Splatting reconstruction, so this covers the whole
//! path — gaussian-splat detection, binary payload parsing, and the conversion of the raw
//! training parameters into a `GaussianSplats3D` archetype.

#[cfg(test)]
mod tests {
    use re_chunk::Chunk;
    use re_importer::{ArchetypeImporter, ImportedData, Importer as _, ImporterSettings};
    use re_sdk_types::archetypes::{GaussianSplats3D, Points3D};

    /// Resolve a path under the workspace-root `tests/assets/` directory.
    ///
    /// `crates/store/re_importer → crates/store → crates → repo-root`.
    fn asset(relative_path: &str) -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(3)
            .expect("workspace root is three ancestors up from crates/store/re_importer")
            .join("tests/assets")
            .join(relative_path)
    }

    fn import_chunks(path: impl AsRef<std::path::Path>) -> Vec<Chunk> {
        let path = path.as_ref().to_path_buf();
        let (tx, rx) = crossbeam::channel::bounded(1024);
        let settings = ImporterSettings::recommended("test");
        ArchetypeImporter
            .import_from_path(&settings, path, tx.clone())
            .unwrap();
        drop(tx);
        rx.iter().filter_map(ImportedData::into_chunk).collect()
    }

    #[test]
    fn test_ply_importer_gaussian_splats() {
        let chunks = import_chunks(asset("gaussian_splats/cactus.ply"));
        assert_eq!(chunks.len(), 1);
        let chunk = &chunks[0];

        // The number of values across all rows of a component column.
        let num_values = |descriptor: re_sdk_types::ComponentDescriptor| {
            chunk
                .components()
                .get(descriptor.component)
                .map(|column| column.list_array.values().len())
        };

        let num_splats = num_values(GaussianSplats3D::descriptor_centers())
            .expect("the importer should have recognized this as gaussian splats");
        assert_eq!(num_splats, 139_410);

        // Everything the `.ply` carries should have made it through.
        for descriptor in [
            GaussianSplats3D::descriptor_scales(),
            GaussianSplats3D::descriptor_quaternions(),
            GaussianSplats3D::descriptor_colors(),
            GaussianSplats3D::descriptor_sh_coefficients(),
        ] {
            assert_eq!(
                num_values(descriptor.clone()),
                Some(num_splats),
                "{descriptor} should have one value per gaussian"
            );
        }

        // A gaussian splat `.ply` must not fall through to the plain point cloud loader.
        assert_eq!(num_values(Points3D::descriptor_positions()), None);
    }
}
