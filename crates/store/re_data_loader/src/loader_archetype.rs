use itertools::Either;

use re_chunk::{Chunk, RowId};
use re_log_types::{EntityPath, TimePoint};
use re_types::archetypes::{AssetVideo, VideoFrameReference};
use re_types::components::VideoTimestamp;
use re_types::Archetype;
use re_types::ComponentBatch;

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

        let contents = {
            re_tracing::profile_scope!("fs::read");
            std::fs::read(&filepath).with_context(|| format!("Failed to read file {filepath:?}"))?
        };
        let contents = std::borrow::Cow::Owned(contents);

        self.load_from_file_contents(settings, filepath, contents, tx)
    }

    fn load_from_file_contents(
        &self,
        settings: &crate::DataLoaderSettings,
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
        } else if crate::SUPPORTED_VIDEO_EXTENSIONS.contains(&extension.as_str()) {
            re_log::debug!(?filepath, loader = self.name(), "Loading video…",);
            rows.extend(load_video(
                &filepath,
                timepoint,
                &entity_path,
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

        let store_id = settings
            .opened_store_id
            .clone()
            .unwrap_or_else(|| settings.store_id.clone());
        for row in rows {
            let data = LoadedData::Chunk(Self::name(&Self), store_id.clone(), row);
            if tx.send(data).is_err() {
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
            let mut arch = re_types::archetypes::EncodedImage::from_file_contents(contents);

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

fn load_video(
    filepath: &std::path::Path,
    mut timepoint: TimePoint,
    entity_path: &EntityPath,
    contents: Vec<u8>,
) -> Result<impl ExactSizeIterator<Item = Chunk>, DataLoaderError> {
    re_tracing::profile_function!();

    let video_timeline = re_log_types::Timeline::new_duration("video");
    timepoint.insert_cell(
        *video_timeline.name(),
        re_log_types::TimeCell::ZERO_DURATION,
    );

    let video_asset = AssetVideo::new(contents);

    let video_frame_reference_chunk = match video_asset.read_frame_timestamps_nanos() {
        Ok(frame_timestamps_nanos) => {
            // Time column.
            let is_sorted = Some(true);
            let frame_timestamps_nanos: arrow::buffer::ScalarBuffer<i64> =
                frame_timestamps_nanos.into();
            let time_column = re_chunk::TimeColumn::new(
                is_sorted,
                video_timeline,
                frame_timestamps_nanos.clone(),
            );

            // VideoTimestamp component column.
            let video_timestamps = frame_timestamps_nanos
                .iter()
                .copied()
                .map(VideoTimestamp::from_nanos)
                .collect::<Vec<_>>();
            let video_timestamp_batch = &video_timestamps as &dyn ComponentBatch;
            let video_timestamp_list_array = video_timestamp_batch
                .to_arrow_list_array()
                .map_err(re_chunk::ChunkError::from)?;

            // Indicator column.
            let video_frame_reference_indicators =
                <VideoFrameReference as Archetype>::Indicator::new_array(video_timestamps.len());
            let video_frame_reference_indicators_list_array = video_frame_reference_indicators
                .to_arrow_list_array()
                .map_err(re_chunk::ChunkError::from)?;

            Some(Chunk::from_auto_row_ids(
                re_chunk::ChunkId::new(),
                entity_path.clone(),
                std::iter::once((*video_timeline.name(), time_column)).collect(),
                [
                    (
                        VideoFrameReference::indicator().descriptor.clone(),
                        video_frame_reference_indicators_list_array,
                    ),
                    (
                        VideoFrameReference::descriptor_timestamp(),
                        video_timestamp_list_array,
                    ),
                ]
                .into_iter()
                .collect(),
            )?)
        }

        Err(err) => {
            re_log::warn_once!(
                "Failed to read frame timestamps from video asset {filepath:?}: {err}"
            );
            None
        }
    };

    // Put video asset into its own chunk since it can be fairly large.
    let video_asset_chunk = Chunk::builder(entity_path.clone())
        .with_archetype(RowId::new(), timepoint.clone(), &video_asset)
        .build()?;

    if let Some(video_frame_reference_chunk) = video_frame_reference_chunk {
        Ok(Either::Left(
            [video_asset_chunk, video_frame_reference_chunk].into_iter(),
        ))
    } else {
        // Still log the video asset, but don't include video frames.
        Ok(Either::Right(std::iter::once(video_asset_chunk)))
    }
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
