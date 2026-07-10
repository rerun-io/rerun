//! Tests for Wavefront OBJ import through the generic archetype importer.

#[cfg(test)]
mod tests {
    use re_chunk::Chunk;
    use re_importer::{ArchetypeImporter, ImportedData, Importer as _, ImporterSettings};
    use re_sdk_types::archetypes::{Asset3D, Mesh3D};
    use re_sdk_types::components::Texcoord2D;
    use re_sdk_types::external::re_types_core::ComponentDescriptor;

    fn load_obj_chunks(path: impl AsRef<std::path::Path>) -> Vec<Chunk> {
        let path = path.as_ref().to_path_buf();
        let (tx, rx) = crossbeam::channel::bounded(1024);
        let settings = ImporterSettings::recommended("test");

        ArchetypeImporter
            .import_from_path(&settings, path, tx.clone())
            .unwrap();
        drop(tx);

        rx.iter().filter_map(ImportedData::into_chunk).collect()
    }

    fn has_descriptor(chunk: &Chunk, descriptor: &ComponentDescriptor) -> bool {
        chunk.component_descriptors().any(|d| d == descriptor)
    }

    fn texcoords(chunk: &Chunk) -> Vec<Texcoord2D> {
        let component = Mesh3D::descriptor_vertex_texcoords().component;
        chunk
            .iter_component::<Texcoord2D>(component)
            .flat_map(|batch| batch.as_slice().to_vec())
            .collect()
    }

    fn write_basic_obj(dir: &std::path::Path, mtl_body: &str) -> std::path::PathBuf {
        let obj_path = dir.join("model.obj");
        std::fs::write(
            &obj_path,
            "\
mtllib model.mtl
o triangle
v 0.0 0.0 0.0
v 1.0 0.0 0.0
v 0.0 1.0 0.0
vn 0.0 0.0 1.0
vt 0.0 0.0
vt 1.0 0.0
vt 0.0 1.0
usemtl material
f 1/1/1 2/2/1 3/3/1
",
        )
        .unwrap();
        std::fs::write(dir.join("model.mtl"), mtl_body).unwrap();
        obj_path
    }

    #[test]
    fn obj_imports_mtl_diffuse_material_as_mesh3d() {
        let dir = tempfile::tempdir().unwrap();
        let obj_path = write_basic_obj(
            dir.path(),
            "\
newmtl material
Kd 0.25 0.50 0.75
d 0.80
",
        );

        let chunks = load_obj_chunks(obj_path);

        assert_eq!(chunks.len(), 1);
        let chunk = &chunks[0];
        assert!(
            has_descriptor(chunk, &Mesh3D::descriptor_vertex_positions()),
            "OBJ imports should emit native Mesh3D data"
        );
        assert!(
            has_descriptor(chunk, &Mesh3D::descriptor_albedo_factor()),
            "Kd/d from the MTL should be logged as the mesh albedo factor"
        );
        assert!(
            !has_descriptor(chunk, &Asset3D::descriptor_blob()),
            "OBJ file imports need resolved sidecar data, not a standalone Asset3D blob"
        );
    }

    #[test]
    fn obj_imports_mtl_diffuse_texture() {
        let dir = tempfile::tempdir().unwrap();
        let texture_path = dir.path().join("texture.png");
        image::RgbImage::from_fn(2, 2, |x, y| image::Rgb([x as u8 * 120, y as u8 * 120, 200]))
            .save(&texture_path)
            .unwrap();

        let obj_path = write_basic_obj(
            dir.path(),
            "\
newmtl material
Kd 1.0 1.0 1.0
map_Kd texture.png
",
        );

        let chunks = load_obj_chunks(obj_path);

        assert_eq!(chunks.len(), 1);
        let chunk = &chunks[0];
        assert!(
            has_descriptor(chunk, &Mesh3D::descriptor_albedo_texture_buffer()),
            "map_Kd from the MTL should be decoded and logged as a mesh texture"
        );
        assert!(
            has_descriptor(chunk, &Mesh3D::descriptor_albedo_texture_format()),
            "decoded mesh textures should include an ImageFormat"
        );

        let texcoords = texcoords(chunk);
        assert_eq!(
            texcoords.iter().map(Texcoord2D::v).collect::<Vec<_>>(),
            vec![1.0, 1.0, 0.0],
            "OBJ texture V coordinates should be converted from lower-left to upper-left origin"
        );
    }
}
