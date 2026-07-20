use re_chunk::{Chunk, RowId};
use re_log_types::{ApplicationId, EntityPath, TimePoint};

use crate::{ImportedData, Importer, ImporterError};

// ---

/// Imports data from any supported file or in-memory contents as native [`re_sdk_types::Archetype`]s.
///
/// This is a simple generic [`Importer`] for filetypes that match 1-to-1 with our builtin
/// archetypes.
pub struct ArchetypeImporter;

impl Importer for ArchetypeImporter {
    #[inline]
    fn name(&self) -> String {
        "rerun.importers.Archetype".into()
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn import_from_path(
        &self,
        settings: &crate::ImporterSettings,
        filepath: std::path::PathBuf,
        tx: crossbeam::channel::Sender<ImportedData>,
    ) -> Result<(), crate::ImporterError> {
        use anyhow::Context as _;

        // NOTE: We're not just checking whether this is or isn't any kind of file here: we
        // are specifically checking whether this is a vanilla, run-of-the-mill, boring file.
        // Not a socket, not a fifo, not some obscure named pipe, and certainly not a symlink to
        // any of these things: just a basic file. Anything other than a vanilla file is assumed to
        // be an RRD stream by default, and therefore will be handled by the RRD importer.
        //
        // This is super important because, if that thing does turn out to be a fifo or something of
        // that nature (e.g. `rerun <(curl …)`), and we end up reading from it, then the RRD importer
        // will end up executing on top of a racy, partial RRD stream (because these virtual streams
        // have process-global state). The end result will be what looks like a bunch of corrupt data and
        // the decoder which will start spewing random confusing errors.
        if !filepath.is_file() {
            return Err(crate::ImporterError::Incompatible(filepath.clone()));
        }

        re_tracing::profile_function!(filepath.display().to_string());

        let contents = {
            re_tracing::profile_scope!("fs::read");
            std::fs::read(&filepath).with_context(|| format!("Failed to read file {filepath:?}"))?
        };
        let contents = std::borrow::Cow::Owned(contents);

        self.import_from_file_contents(settings, filepath, contents, tx)
    }

    fn import_from_file_contents(
        &self,
        settings: &crate::ImporterSettings,
        filepath: std::path::PathBuf,
        contents: std::borrow::Cow<'_, [u8]>,
        tx: crossbeam::channel::Sender<ImportedData>,
    ) -> Result<(), crate::ImporterError> {
        let extension = crate::extension(&filepath);
        if !crate::is_supported_file_extension(&extension) {
            return Err(crate::ImporterError::Incompatible(filepath.clone()));
        }

        re_tracing::profile_function!(filepath.display().to_string());

        let entity_path = settings
            .entity_path_prefix
            .clone()
            .map(|prefix| prefix / EntityPath::from_file_path(&filepath))
            .unwrap_or_else(|| EntityPath::from_file_path(&filepath));

        #[cfg_attr(target_arch = "wasm32", expect(unused_mut))]
        let mut timepoint = TimePoint::default();

        #[cfg(not(target_arch = "wasm32"))]
        if false && let Ok(metadata) = filepath.metadata() {
            // TODO(cmc): log these once heuristics (I think?) are fixed
            use re_log_types::TimeCell;

            if let Some(created) = metadata
                .created()
                .ok()
                .and_then(|t| TimeCell::try_from(t).ok())
            {
                timepoint.insert_cell("created_at", created);
            }
            if let Some(modified) = metadata
                .modified()
                .ok()
                .and_then(|t| TimeCell::try_from(t).ok())
            {
                timepoint.insert_cell("modified_at", modified);
            }
            if let Some(accessed) = metadata
                .accessed()
                .ok()
                .and_then(|t| TimeCell::try_from(t).ok())
            {
                timepoint.insert_cell("accessed_at", accessed);
            }
        }

        let store_id = settings.opened_store_id.clone().unwrap_or_else(|| {
            re_log_types::StoreId::recording(
                settings
                    .application_id
                    .clone()
                    .unwrap_or_else(ApplicationId::random),
                settings.recording_id.clone(),
            )
        });

        // We stream chunks to `tx` as each loader produces them, rather than
        // collecting into a `Vec` first: that way the bounded channel's
        // backpressure actually limits how much is held in memory at once.
        let num_chunks = if crate::SUPPORTED_IMAGE_EXTENSIONS.contains(&extension.as_str()) {
            re_log::debug!(?filepath, importer = self.name(), "Loading image…",);
            self.send_chunks(
                &tx,
                &store_id,
                load_image(&filepath, timepoint, entity_path, contents.into_owned())?,
            )
        } else if crate::SUPPORTED_DEPTH_IMAGE_EXTENSIONS.contains(&extension.as_str()) {
            re_log::debug!(?filepath, importer = self.name(), "Loading depth image…",);
            self.send_chunks(
                &tx,
                &store_id,
                load_depth_image(&filepath, timepoint, entity_path, contents.into_owned())?,
            )
        } else if crate::SUPPORTED_VIDEO_EXTENSIONS.contains(&extension.as_str()) {
            re_log::debug!(?filepath, importer = self.name(), "Loading video…",);
            self.send_chunks(
                &tx,
                &store_id,
                load_video(&filepath, timepoint, &entity_path, contents.into_owned())?,
            )
        } else if crate::SUPPORTED_MESH_EXTENSIONS.contains(&extension.as_str()) {
            re_log::debug!(?filepath, importer = self.name(), "Loading 3D model…",);
            self.send_chunks(
                &tx,
                &store_id,
                load_mesh(
                    filepath.clone(),
                    timepoint,
                    entity_path,
                    contents.into_owned(),
                )?,
            )
        } else if crate::SUPPORTED_POINT_CLOUD_EXTENSIONS.contains(&extension.as_str()) {
            re_log::debug!(?filepath, importer = self.name(), "Loading .ply geometry…",);
            self.send_chunks(&tx, &store_id, load_ply(timepoint, entity_path, &contents)?)
        } else if crate::SUPPORTED_TEXT_EXTENSIONS.contains(&extension.as_str()) {
            re_log::debug!(?filepath, importer = self.name(), "Loading text document…",);
            self.send_chunks(
                &tx,
                &store_id,
                load_text_document(
                    filepath.clone(),
                    timepoint,
                    entity_path,
                    contents.into_owned(),
                )?,
            )
        } else {
            return Err(crate::ImporterError::Incompatible(filepath.clone()));
        };

        if num_chunks == 0 {
            re_log::warn!("{} is empty", filepath.display());
        }

        Ok(())
    }
}

impl ArchetypeImporter {
    /// Streams `chunks` to `tx`, returning the number actually sent.
    ///
    /// Stops early (without erroring) if the receiver has hung up — that just
    /// means the import is being torn down, which is not our problem.
    fn send_chunks(
        &self,
        tx: &crossbeam::channel::Sender<ImportedData>,
        store_id: &re_log_types::StoreId,
        chunks: impl Iterator<Item = Chunk>,
    ) -> usize {
        let mut num_sent = 0;
        for chunk in chunks {
            let data = ImportedData::Chunk(self.name(), store_id.clone(), chunk);
            if re_quota_channel::send_crossbeam(tx, data).is_err() {
                break;
            }
            num_sent += 1;
        }
        num_sent
    }
}

// ---

fn load_image(
    filepath: &std::path::Path,
    timepoint: TimePoint,
    entity_path: EntityPath,
    contents: Vec<u8>,
) -> Result<impl ExactSizeIterator<Item = Chunk> + use<>, ImporterError> {
    re_tracing::profile_function!();

    let rows = [
        {
            let mut arch = re_sdk_types::archetypes::EncodedImage::from_file_contents(contents);

            if let Ok(format) = image::ImageFormat::from_path(filepath) {
                arch = arch.with_media_type(format.to_mime_type());
            }

            Chunk::builder(entity_path)
                .with_archetype(RowId::new(), timepoint, &arch)
                .build()?
        },
        //
    ];

    Ok(rows.into_iter())
}

fn load_depth_image(
    filepath: &std::path::Path,
    timepoint: TimePoint,
    entity_path: EntityPath,
    contents: Vec<u8>,
) -> Result<impl ExactSizeIterator<Item = Chunk> + use<>, ImporterError> {
    re_tracing::profile_function!();

    let rows = [{
        let mut arch = re_sdk_types::archetypes::EncodedDepthImage::from_file_contents(contents);

        if let Ok(format) = image::ImageFormat::from_path(filepath) {
            arch = arch.with_media_type(format.to_mime_type());
        } else if filepath
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.to_lowercase() == "rvl")
        {
            arch = arch.with_media_type(re_sdk_types::components::MediaType::RVL);
        }

        Chunk::builder(entity_path)
            .with_archetype(RowId::new(), timepoint, &arch)
            .build()?
    }];

    Ok(rows.into_iter())
}

fn load_video(
    filepath: &std::path::Path,
    mut timepoint: TimePoint,
    entity_path: &EntityPath,
    contents: Vec<u8>,
) -> Result<impl Iterator<Item = Chunk> + use<>, ImporterError> {
    re_tracing::profile_function!();

    let video_timeline = re_log_types::Timeline::new_duration("video");
    timepoint.insert_cell(
        *video_timeline.name(),
        re_log_types::TimeCell::ZERO_DURATION,
    );

    let config = re_mp4_reader::Mp4Config {
        mode: re_mp4_reader::Mode::Asset { timepoint },
        timeline_name: "video".into(),
        timeline_type: re_log_types::TimeType::DurationNs,
        ffmpeg_override: None,
    };

    // An up-front failure (e.g. the asset being too large) aborts the import.
    let debug_name = filepath.display().to_string();
    let chunks = re_mp4_reader::load_mp4_from_bytes(contents, &config, entity_path, &debug_name)
        .map_err(|err| ImporterError::Mp4 {
            path: filepath.to_path_buf(),
            source: err,
        })?;

    // The returned iterator is lazy — chunks are constructed one at a time as
    // the caller drains it. A per-chunk failure is *not* fatal: we log it and
    // stop, keeping whatever chunks were produced before it (partial import).
    // (Unreadable frame timestamps are not an error at all — `re_mp4_reader`
    // handles that leniently by emitting only the asset chunk.)
    let filepath = filepath.to_path_buf();
    Ok(chunks.map_while(move |chunk| match chunk {
        Ok(chunk) => Some(chunk),
        Err(err) => {
            re_log::warn!(?filepath, "Failed to load chunk from video: {err}");
            None
        }
    }))
}

fn load_mesh(
    filepath: std::path::PathBuf,
    timepoint: TimePoint,
    entity_path: EntityPath,
    contents: Vec<u8>,
) -> Result<impl ExactSizeIterator<Item = Chunk>, ImporterError> {
    re_tracing::profile_function!();

    let rows = [
        {
            let arch = re_sdk_types::archetypes::Asset3D::from_file_contents(
                contents,
                re_sdk_types::components::MediaType::guess_from_path(filepath),
            );
            Chunk::builder(entity_path)
                .with_archetype(RowId::new(), timepoint, &arch)
                .build()?
        },
        //
    ];

    Ok(rows.into_iter())
}

fn load_ply(
    timepoint: TimePoint,
    entity_path: EntityPath,
    contents: &[u8],
) -> Result<impl ExactSizeIterator<Item = Chunk> + use<>, ImporterError> {
    re_tracing::profile_function!();

    let rows = [
        {
            match re_ply::classify_geometry_from_bytes(contents).map_err(anyhow::Error::from)? {
                re_ply::PlyGeometryClass::Points2D => {
                    let points2d = re_sdk_types::archetypes::Points2D::from_file_contents(contents)
                        .map_err(anyhow::Error::from)?;
                    Chunk::builder(entity_path)
                        .with_archetype(RowId::new(), timepoint, &points2d)
                        .build()?
                }
                re_ply::PlyGeometryClass::Points3D => {
                    let points3d = re_sdk_types::archetypes::Points3D::from_file_contents(contents)
                        .map_err(anyhow::Error::from)?;
                    Chunk::builder(entity_path)
                        .with_archetype(RowId::new(), timepoint, &points3d)
                        .build()?
                }
                re_ply::PlyGeometryClass::MeshOrAsset3D => {
                    let asset3d = re_sdk_types::archetypes::Asset3D::from_file_contents(
                        contents.to_vec(),
                        Some(re_sdk_types::components::MediaType::ply()),
                    );
                    Chunk::builder(entity_path)
                        .with_archetype(RowId::new(), timepoint, &asset3d)
                        .build()?
                }
            }
        },
        //
    ];

    Ok(rows.into_iter())
}

fn load_text_document(
    filepath: std::path::PathBuf,
    timepoint: TimePoint,
    entity_path: EntityPath,
    contents: Vec<u8>,
) -> Result<impl ExactSizeIterator<Item = Chunk>, ImporterError> {
    re_tracing::profile_function!();

    let rows = [
        {
            let arch = re_sdk_types::archetypes::TextDocument::from_file_contents(
                contents,
                re_sdk_types::components::MediaType::guess_from_path(filepath),
            )
            .map_err(anyhow::Error::from)?;
            Chunk::builder(entity_path)
                .with_archetype(RowId::new(), timepoint, &arch)
                .build()?
        },
        //
    ];

    Ok(rows.into_iter())
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use super::{ArchetypeImporter, load_ply};
    use re_chunk::{Chunk, ChunkComponents, RowId};
    use re_log_types::{EntityPath, TimePoint};
    use re_sdk_types::archetypes::{Asset3D, Mesh3D, Points2D, Points3D};
    use re_sdk_types::components::MediaType;

    use crate::{ImportedData, Importer as _, ImporterSettings};

    fn load_single_chunk(contents: &[u8]) -> Chunk {
        let chunks = load_ply(TimePoint::default(), EntityPath::from("points"), contents)
            .unwrap()
            .collect::<Vec<_>>();

        assert_eq!(chunks.len(), 1);
        chunks.into_iter().next().unwrap()
    }

    fn assert_ply_asset3d_chunk(chunk: &Chunk, entity_path: &str, contents: &[u8]) {
        assert!(
            chunk
                .components()
                .contains_component(Asset3D::descriptor_blob().component)
        );
        assert!(
            !chunk
                .components()
                .contains_component(Mesh3D::descriptor_vertex_positions().component)
        );
        assert!(
            !chunk
                .components()
                .contains_component(Points2D::descriptor_positions().component)
        );
        assert!(
            !chunk
                .components()
                .contains_component(Points3D::descriptor_positions().component)
        );

        let expected = Chunk::builder(EntityPath::from(entity_path))
            .with_archetype(
                RowId::new(),
                TimePoint::default(),
                &Asset3D::from_file_contents(contents.to_vec(), Some(MediaType::ply())),
            )
            .build()
            .unwrap();

        ChunkComponents::ensure_similar(expected.components(), chunk.components()).unwrap();
    }

    fn load_single_chunk_via_archetype_importer(filepath: &str, contents: &[u8]) -> Chunk {
        let importer = ArchetypeImporter;
        let (tx, rx) = crossbeam::channel::bounded(8);
        let settings = ImporterSettings::recommended("test");

        importer
            .import_from_file_contents(&settings, filepath.into(), Cow::Borrowed(contents), tx)
            .unwrap();

        let chunks = rx
            .into_iter()
            .filter_map(ImportedData::into_chunk)
            .collect::<Vec<_>>();

        assert_eq!(chunks.len(), 1);
        chunks.into_iter().next().unwrap()
    }

    #[test]
    fn ply_xy_loads_as_points2d() {
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

        let chunk = load_single_chunk(contents);

        assert!(
            chunk
                .components()
                .contains_component(Points2D::descriptor_positions().component)
        );
        assert!(
            !chunk
                .components()
                .contains_component(Points3D::descriptor_positions().component)
        );
        assert!(
            !chunk
                .components()
                .contains_component(Asset3D::descriptor_blob().component)
        );

        let expected = Chunk::builder(EntityPath::from("points"))
            .with_archetype(
                RowId::new(),
                TimePoint::default(),
                &Points2D::new([(1.0, 2.0), (4.0, 5.0)]).with_colors([0x0A141EFF, 0x0B151FFF]),
            )
            .build()
            .unwrap();

        ChunkComponents::ensure_similar(expected.components(), chunk.components()).unwrap();
    }

    #[test]
    fn archetype_loader_routes_ply_xy_to_points2d() {
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

        let chunk = load_single_chunk_via_archetype_importer("points_xy.ply", contents);

        assert!(
            chunk
                .components()
                .contains_component(Points2D::descriptor_positions().component)
        );
        assert!(
            !chunk
                .components()
                .contains_component(Points3D::descriptor_positions().component)
        );
        assert!(
            !chunk
                .components()
                .contains_component(Mesh3D::descriptor_vertex_positions().component)
        );
        assert!(
            !chunk
                .components()
                .contains_component(Asset3D::descriptor_blob().component)
        );
    }

    #[test]
    fn archetype_loader_routes_ply_xyz_to_points3d() {
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

        let chunk = load_single_chunk_via_archetype_importer("points_xyz.ply", contents);

        assert!(
            chunk
                .components()
                .contains_component(Points3D::descriptor_positions().component)
        );
        assert!(
            !chunk
                .components()
                .contains_component(Points2D::descriptor_positions().component)
        );
        assert!(
            !chunk
                .components()
                .contains_component(Mesh3D::descriptor_vertex_positions().component)
        );
        assert!(
            !chunk
                .components()
                .contains_component(Asset3D::descriptor_blob().component)
        );
    }

    #[test]
    fn archetype_loader_routes_ply_mesh_to_asset3d() {
        let contents = br#"ply
format ascii 1.0
element vertex 4
property float x
property float y
property float z
property uchar red
property uchar green
property uchar blue
element face 1
property list uchar int vertex_indices
end_header
0 0 0 255 0 0
1 0 0 0 255 0
1 1 0 0 0 255
0 1 0 255 255 0
4 0 1 2 3
"#;

        let chunk = load_single_chunk_via_archetype_importer("mesh.ply", contents);

        assert!(
            chunk
                .components()
                .contains_component(Asset3D::descriptor_blob().component)
        );
        assert!(
            !chunk
                .components()
                .contains_component(Mesh3D::descriptor_vertex_positions().component)
        );
        assert!(
            !chunk
                .components()
                .contains_component(Points3D::descriptor_positions().component)
        );
        assert!(
            !chunk
                .components()
                .contains_component(Points2D::descriptor_positions().component)
        );
    }

    #[test]
    fn ply_xyz_faces_load_as_asset3d() {
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

        let chunk = load_single_chunk(contents);

        assert_ply_asset3d_chunk(&chunk, "points", contents);
    }

    #[test]
    fn ply_xy_faces_load_as_asset3d() {
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

        let chunk = load_single_chunk(contents);

        assert_ply_asset3d_chunk(&chunk, "points", contents);
    }

    #[test]
    fn ply_with_topology_loads_as_asset3d_without_point_validation() {
        let contents = br#"ply
format ascii 1.0
element face 1
property list uchar int vertex_indices
end_header
3 0 1 2
"#;

        let chunk = load_single_chunk(contents);

        assert_ply_asset3d_chunk(&chunk, "points", contents);
    }

    #[test]
    fn ply_nonempty_unsupported_faces_load_as_asset3d() {
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

        let chunk = load_single_chunk(contents);

        assert_ply_asset3d_chunk(&chunk, "points", contents);
    }

    #[test]
    fn ply_xy_zero_faces_load_as_points2d() {
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

        let chunk = load_single_chunk(contents);

        assert!(
            chunk
                .components()
                .contains_component(Points2D::descriptor_positions().component)
        );
        assert!(
            !chunk
                .components()
                .contains_component(Mesh3D::descriptor_vertex_positions().component)
        );
        assert!(
            !chunk
                .components()
                .contains_component(Asset3D::descriptor_blob().component)
        );

        let expected = Chunk::builder(EntityPath::from("points"))
            .with_archetype(
                RowId::new(),
                TimePoint::default(),
                &Points2D::new([(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)])
                    .with_colors([0xFF0000FF, 0x00FF00FF, 0x0000FFFF, 0xFFFF00FF]),
            )
            .build()
            .unwrap();

        ChunkComponents::ensure_similar(expected.components(), chunk.components()).unwrap();
    }

    #[test]
    fn ply_nonempty_unsupported_faces_load_as_asset3d_without_point_validation() {
        let contents = br#"ply
format ascii 1.0
element face 1
property int material_index
end_header
7
"#;

        let chunk = load_single_chunk(contents);

        assert_ply_asset3d_chunk(&chunk, "points", contents);
    }

    #[test]
    fn ply_without_vertex_element_or_topology_is_rejected() {
        let contents = br#"ply
format ascii 1.0
element material 1
property int material_index
end_header
7
"#;

        let Err(err) = load_ply(TimePoint::default(), EntityPath::from("points"), contents) else {
            panic!("expected missing vertex element to be rejected");
        };

        assert!(
            err.to_string()
                .contains("PLY file is missing required \"vertex\" element")
        );
    }

    #[test]
    fn ply_without_y_coordinate_is_rejected() {
        let contents = br#"ply
format ascii 1.0
element vertex 2
property float x
property float z
end_header
1 2
4 5
"#;

        let Err(err) = load_ply(TimePoint::default(), EntityPath::from("points"), contents) else {
            panic!("expected missing y coordinate to be rejected");
        };

        assert!(
            err.to_string()
                .contains("PLY vertex element must contain at least \"x\" and \"y\"")
        );
    }
}
