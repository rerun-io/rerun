//! End-to-end tests of the `.ply` importer: contents → `ArchetypeImporter` → chunks.
//!
//! `.ply` is versatile enough to hold a 2D or 3D point cloud, a mesh, or a gaussian splat
//! reconstruction, so what matters here is which archetype a given header ends up as.
//! Header classification is unit-tested in `re_importer`'s `ply` module, and payload
//! parsing in `re_sdk_types`.

#[cfg(test)]
mod tests {
    use std::path::Path;

    use re_chunk::{Chunk, ChunkComponents, RowId};
    use re_importer::{ArchetypeImporter, ImportedData, Importer as _, ImporterSettings};
    use re_log_types::{EntityPath, TimePoint};
    use re_sdk_types::AsComponents;
    use re_sdk_types::archetypes::{Asset3D, GaussianSplats3D, Points2D, Points3D};
    use re_sdk_types::components::MediaType;

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

    /// Import `contents` as if it were the file at `filepath`.
    fn import_single_chunk(filepath: &str, contents: &[u8]) -> Chunk {
        let (tx, rx) = crossbeam::channel::bounded(8);
        let settings = ImporterSettings::recommended("test");

        ArchetypeImporter
            .import_from_file_contents(
                &settings,
                filepath.into(),
                std::borrow::Cow::Borrowed(contents),
                tx,
            )
            .unwrap();

        let chunks = rx
            .into_iter()
            .filter_map(ImportedData::into_chunk)
            .collect::<Vec<_>>();

        assert_eq!(chunks.len(), 1);
        chunks.into_iter().next().unwrap()
    }

    /// Assert that importing `contents` yields exactly the components of `expected`.
    ///
    /// `ensure_similar` compares the whole component set, so this also pins down what the
    /// file must *not* have been read as.
    fn assert_imports_as(filepath: &str, contents: &[u8], expected: &impl AsComponents) {
        let chunk = import_single_chunk(filepath, contents);

        let expected = Chunk::builder(EntityPath::from_file_path(Path::new(filepath)))
            .with_archetype(RowId::new(), TimePoint::default(), expected)
            .build()
            .unwrap();

        ChunkComponents::ensure_similar(expected.components(), chunk.components()).unwrap();
    }

    #[test]
    fn xy_vertices_load_as_points2d() {
        let contents = br#"ply
format ascii 1.0
element vertex 2
property float x
property float y
property uchar red
property uchar green
property uchar blue
end_header
1 2 10 20 30
4 5 11 21 31
"#;

        assert_imports_as(
            "points_xy.ply",
            contents,
            &Points2D::new([(1.0, 2.0), (4.0, 5.0)]).with_colors([0x0A141EFF, 0x0B151FFF]),
        );
    }

    #[test]
    fn xyz_vertices_load_as_points3d() {
        let contents = br#"ply
format ascii 1.0
element vertex 2
property float x
property float y
property float z
property uchar red
property uchar green
property uchar blue
end_header
1 2 3 10 20 30
4 5 6 11 21 31
"#;

        assert_imports_as(
            "points_xyz.ply",
            contents,
            &Points3D::new([(1.0, 2.0, 3.0), (4.0, 5.0, 6.0)])
                .with_colors([0x0A141EFF, 0x0B151FFF]),
        );
    }

    /// Topology means the viewer has to render it, so we hand the bytes over untouched.
    #[test]
    fn xyz_faces_load_as_asset3d() {
        let contents = br#"ply
format ascii 1.0
element vertex 4
property float x
property float y
property float z
property float nx
property float ny
property float nz
property uchar red
property uchar green
property uchar blue
element face 1
property list uchar int vertex_indices
end_header
0 0 0 0 0 1 255 0 0
1 0 0 0 0 1 0 255 0
1 1 0 0 0 1 0 0 255
0 1 0 0 0 1 255 255 0
4 0 1 2 3
"#;

        assert_imports_as(
            "mesh_xyz.ply",
            contents,
            &Asset3D::from_file_contents(contents.to_vec(), Some(MediaType::ply())),
        );
    }

    /// A flat mesh is still a mesh; the viewer flattens it onto `z = 0`.
    #[test]
    fn xy_faces_load_as_asset3d() {
        let contents = br#"ply
format ascii 1.0
element vertex 4
property float x
property float y
property uchar red
property uchar green
property uchar blue
element face 1
property list uchar int vertex_indices
end_header
0 0 255 0 0
1 0 0 255 0
1 1 0 0 255
0 1 255 255 0
4 0 1 2 3
"#;

        assert_imports_as(
            "mesh_xy.ply",
            contents,
            &Asset3D::from_file_contents(contents.to_vec(), Some(MediaType::ply())),
        );
    }

    /// Faces alone are enough: we never validate the vertices of something we pass through.
    #[test]
    fn faces_without_vertices_load_as_asset3d() {
        let contents = br#"ply
format ascii 1.0
element face 1
property list uchar int vertex_indices
end_header
3 0 1 2
"#;

        assert_imports_as(
            "faces_only.ply",
            contents,
            &Asset3D::from_file_contents(contents.to_vec(), Some(MediaType::ply())),
        );
    }

    /// A face element we cannot read the indices of is still topology, so still an asset.
    #[test]
    fn unreadable_face_indices_still_load_as_asset3d() {
        let contents = br#"ply
format ascii 1.0
element vertex 4
property float x
property float y
property uchar red
property uchar green
property uchar blue
element face 1
property int material_index
end_header
0 0 255 0 0
1 0 0 255 0
1 1 0 0 255
0 1 255 255 0
7
"#;

        assert_imports_as(
            "unsupported_faces.ply",
            contents,
            &Asset3D::from_file_contents(contents.to_vec(), Some(MediaType::ply())),
        );
    }

    /// A declared but empty face element carries no topology, so this is a point cloud.
    #[test]
    fn zero_faces_load_as_points2d() {
        let contents = br#"ply
format ascii 1.0
element vertex 4
property float x
property float y
property uchar red
property uchar green
property uchar blue
element face 0
property list uchar int vertex_indices
end_header
0 0 255 0 0
1 0 0 255 0
1 1 0 0 255
0 1 255 255 0
"#;

        assert_imports_as(
            "zero_faces.ply",
            contents,
            &Points2D::new([(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)])
                .with_colors([0xFF0000FF, 0x00FF00FF, 0x0000FFFF, 0xFFFF00FF]),
        );
    }

    /// `cactus.ply` is a real 3D Gaussian Splatting reconstruction, so this covers the whole
    /// path — gaussian-splat detection, binary payload parsing, and the conversion of the raw
    /// training parameters into a `GaussianSplats3D` archetype.
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
