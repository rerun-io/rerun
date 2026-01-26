use itertools::Either;
use re_chunk::{Chunk, RowId};
use re_log_types::{ApplicationId, EntityPath, TimePoint};
use re_sdk_types::ComponentBatch;
use re_sdk_types::archetypes::{AssetVideo, VideoFrameReference};
use re_sdk_types::components::VideoTimestamp;

use crate::{DataLoader, DataLoaderError, LoadedData};

// ---

/// Loads data from any supported file or in-memory contents as native [`re_sdk_types::Archetype`]s.
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
        tx: crossbeam::channel::Sender<LoadedData>,
    ) -> Result<(), crate::DataLoaderError> {
        use anyhow::Context as _;

        // NOTE: We're not just checking whether this is or isn't any kind of file here: we
        // are specifically checking whether this is a vanilla, run-of-the-mill, boring file.
        // Not a socket, not a fifo, not some obscure named pipe, and certainly not a symlink to
        // any of these things: just a basic file. Anything other than a vanilla file is assumed to
        // be an RRD stream by default, and therefore will be handled by the RRD data loader.
        //
        // This is super important because, if that thing does turn out to be a fifo or something of
        // that nature (e.g. `rerun <(curl …)`), and we end up reading from it, then the RRD data loader
        // will end up executing on top of a racy, partial RRD stream (because these virtual streams
        // have process-global state). The end result will be what looks like a bunch of corrupt data and
        // the decoder which will start spewing random confusing errors.
        if !filepath.is_file() {
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
        tx: crossbeam::channel::Sender<LoadedData>,
    ) -> Result<(), crate::DataLoaderError> {
        let extension = crate::extension(&filepath);
        if !crate::is_supported_file_extension(&extension) {
            return Err(crate::DataLoaderError::Incompatible(filepath.clone()));
        }

        re_tracing::profile_function!(filepath.display().to_string());

        let entity_path = settings
            .entity_path_prefix
            .clone()
            .map(|prefix| prefix / EntityPath::from_file_path(&filepath))
            .unwrap_or_else(|| EntityPath::from_file_path(&filepath));

        let mut timepoint = TimePoint::default();
        // TODO(cmc): log these once heuristics (I think?) are fixed
        if false && let Ok(metadata) = filepath.metadata() {
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

        let mut rows = Vec::new();

        if crate::SUPPORTED_IMAGE_EXTENSIONS.contains(&extension.as_str()) {
            re_log::debug!(?filepath, loader = self.name(), "Loading image…",);
            rows.extend(load_image(
                &filepath,
                timepoint,
                entity_path,
                contents.into_owned(),
            )?);
        } else if crate::SUPPORTED_DEPTH_IMAGE_EXTENSIONS.contains(&extension.as_str()) {
            re_log::debug!(?filepath, loader = self.name(), "Loading depth image…",);
            rows.extend(load_depth_image(
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

        let store_id = settings.opened_store_id.clone().unwrap_or_else(|| {
            re_log_types::StoreId::recording(
                settings
                    .application_id
                    .clone()
                    .unwrap_or_else(ApplicationId::random),
                settings.recording_id.clone(),
            )
        });
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
) -> Result<impl ExactSizeIterator<Item = Chunk> + use<>, DataLoaderError> {
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
) -> Result<impl ExactSizeIterator<Item = Chunk> + use<>, DataLoaderError> {
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
) -> Result<impl ExactSizeIterator<Item = Chunk> + use<>, DataLoaderError> {
    re_tracing::profile_function!();

    let video_timeline = re_log_types::Timeline::new_duration("video");
    timepoint.insert_cell(
        *video_timeline.name(),
        re_log_types::TimeCell::ZERO_DURATION,
    );

    let video_asset = {
        re_tracing::profile_scope!("serialize-as-arrow");
        AssetVideo::new(contents)
    };

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

            Some(Chunk::from_auto_row_ids(
                re_chunk::ChunkId::new(),
                entity_path.clone(),
                std::iter::once((*video_timeline.name(), time_column)).collect(),
                std::iter::once((
                    VideoFrameReference::descriptor_timestamp(),
                    video_timestamp_list_array,
                ))
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

fn load_point_cloud(
    timepoint: TimePoint,
    entity_path: EntityPath,
    contents: &[u8],
) -> Result<impl ExactSizeIterator<Item = Chunk> + use<>, DataLoaderError> {
    re_tracing::profile_function!();

    let rows = [
        {
            // TODO(#4532): `.ply` data loader should support 2D point cloud & meshes
            let points3d = re_sdk_types::archetypes::Points3D::from_file_contents(contents)?;
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
            let arch = re_sdk_types::archetypes::TextDocument::from_file_contents(
                contents,
                re_sdk_types::components::MediaType::guess_from_path(filepath),
            )?;
            Chunk::builder(entity_path)
                .with_archetype(RowId::new(), timepoint, &arch)
                .build()?
        },
        //
    ];

    Ok(rows.into_iter())
}
