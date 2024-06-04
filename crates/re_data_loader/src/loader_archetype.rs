use re_chunk::Chunk;
use re_log_types::{EntityPath, RowId, TimeInt, TimePoint};

use crate::{DataLoader, DataLoaderError, LoadedData};

// ---

/// Loads data from any supported file or in-memory contents as native [`re_types::Archetype`]s.
///
/// This is a simple generic [`DataLoader`] for filetypes that match 1-to-1 with our builtin
/// archetypes.
pub struct ArchetypeLoader;

impl DataLoader for ArchetypeLoader {
    #[inline]
    fn name(&self) -> String {
        "rerun.data_loaders.Archetype".into()
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn load_from_path(
        &self,
        settings: &crate::DataLoaderSettings,
        filepath: std::path::PathBuf,
        tx: std::sync::mpsc::Sender<LoadedData>,
    ) -> Result<(), crate::DataLoaderError> {
        use anyhow::Context as _;

        if filepath.is_dir() {
            return Err(crate::DataLoaderError::Incompatible(filepath.clone()));
        }

        re_tracing::profile_function!(filepath.display().to_string());

        let contents = std::fs::read(&filepath)
            .with_context(|| format!("Failed to read file {filepath:?}"))?;
        let contents = std::borrow::Cow::Owned(contents);

        self.load_from_file_contents(settings, filepath, contents, tx)
    }

    fn load_from_file_contents(
        &self,
        _settings: &crate::DataLoaderSettings,
        filepath: std::path::PathBuf,
        contents: std::borrow::Cow<'_, [u8]>,
        tx: std::sync::mpsc::Sender<LoadedData>,
    ) -> Result<(), crate::DataLoaderError> {
        let extension = crate::extension(&filepath);
        if !crate::is_supported_file_extension(&extension) {
            return Err(crate::DataLoaderError::Incompatible(filepath.clone()));
        }

        re_tracing::profile_function!(filepath.display().to_string());

        let entity_path = EntityPath::from_file_path(&filepath);

        let mut timepoint = TimePoint::default();
        // TODO(cmc): log these once heuristics (I think?) are fixed
        if false {
            if let Ok(metadata) = filepath.metadata() {
                use re_log_types::{Time, Timeline};

                if let Some(created) = metadata
                    .created()
                    .ok()
                    .and_then(|t| TimeInt::try_from(Time::try_from(t).ok()?).ok())
                {
                    timepoint.insert(Timeline::new_temporal("created_at"), created);
                }
                if let Some(modified) = metadata
                    .modified()
                    .ok()
                    .and_then(|t| TimeInt::try_from(Time::try_from(t).ok()?).ok())
                {
                    timepoint.insert(Timeline::new_temporal("modified_at"), modified);
                }
                if let Some(accessed) = metadata
                    .accessed()
                    .ok()
                    .and_then(|t| TimeInt::try_from(Time::try_from(t).ok()?).ok())
                {
                    timepoint.insert(Timeline::new_temporal("accessed_at"), accessed);
                }
            }
        }

        let mut rows = Vec::new();

        if crate::SUPPORTED_IMAGE_EXTENSIONS.contains(&extension.as_str()) {
            re_log::debug!(?filepath, loader = self.name(), "Loading image…",);
            rows.extend(load_image(
                &filepath,
                timepoint,
                entity_path,
                contents.into_owned(),
            )?);
        } else if crate::SUPPORTED_MESH_EXTENSIONS.contains(&extension.as_str()) {
            re_log::debug!(?filepath, loader = self.name(), "Loading 3D model…",);
            rows.extend(load_mesh(
                filepath,
                timepoint,
                entity_path,
                contents.into_owned(),
            )?);
        } else if crate::SUPPORTED_POINT_CLOUD_EXTENSIONS.contains(&extension.as_str()) {
            re_log::debug!(?filepath, loader = self.name(), "Loading 3D point cloud…",);
            rows.extend(load_point_cloud(timepoint, entity_path, &contents)?);
        } else if crate::SUPPORTED_TEXT_EXTENSIONS.contains(&extension.as_str()) {
            re_log::debug!(?filepath, loader = self.name(), "Loading text document…",);
            rows.extend(load_text_document(
                filepath,
                timepoint,
                entity_path,
                contents.into_owned(),
            )?);
        }

        for row in rows {
            if tx.send(row.into()).is_err() {
                break; // The other end has decided to hang up, not our problem.
            }
        }

        Ok(())
    }
}

// ---

fn load_image(
    filepath: &std::path::Path,
    timepoint: TimePoint,
    entity_path: EntityPath,
    contents: Vec<u8>,
) -> Result<impl ExactSizeIterator<Item = Chunk>, DataLoaderError> {
    re_tracing::profile_function!();

    let rows = [
        {
            let arch = re_types::archetypes::Image::from_file_contents(
                contents,
                image::ImageFormat::from_path(filepath).ok(),
            )?;
            Chunk::builder(entity_path)
                .with_archetype(RowId::new(), timepoint, &arch)
                .build()?
        },
        //
    ];

    Ok(rows.into_iter())
}

fn load_mesh(
    filepath: std::path::PathBuf,
    timepoint: TimePoint,
    entity_path: EntityPath,
    contents: Vec<u8>,
) -> Result<impl ExactSizeIterator<Item = Chunk>, DataLoaderError> {
    re_tracing::profile_function!();

    let rows = [
        {
            let arch = re_types::archetypes::Asset3D::from_file_contents(
                contents,
                re_types::components::MediaType::guess_from_path(filepath),
            );
            Chunk::builder(entity_path)
                .with_archetype(RowId::new(), timepoint, &arch)
                .build()?
        },
        //
    ];

    Ok(rows.into_iter())
}

fn load_point_cloud(
    timepoint: TimePoint,
    entity_path: EntityPath,
    contents: &[u8],
) -> Result<impl ExactSizeIterator<Item = Chunk>, DataLoaderError> {
    re_tracing::profile_function!();

    let rows = [
        {
            // TODO(#4532): `.ply` data loader should support 2D point cloud & meshes
            let points3d = re_types::archetypes::Points3D::from_file_contents(contents)?;
            Chunk::builder(entity_path)
                .with_archetype(RowId::new(), timepoint, &points3d)
                .build()?
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
) -> Result<impl ExactSizeIterator<Item = Chunk>, DataLoaderError> {
    re_tracing::profile_function!();

    let rows = [
        {
            let arch = re_types::archetypes::TextDocument::from_file_contents(
                contents,
                re_types::components::MediaType::guess_from_path(filepath),
            )?;
            Chunk::builder(entity_path)
                .with_archetype(RowId::new(), timepoint, &arch)
                .build()?
        },
        //
    ];

    Ok(rows.into_iter())
}
