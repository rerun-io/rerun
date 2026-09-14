use re_chunk::Chunk;

use crate::{
    LeRobotError,
    common::{
        build_text_chunk, build_video_asset_chunks, build_video_stream_chunks,
        load_episode_depth_images, load_episode_images, load_scalar,
    },
    plan::{EpisodePlan, PlannedFeature, PlannedVideo},
};

/// Turn one planned feature into Rerun chunks, decoding from the plan's record batch or
/// reading from disk as needed.
pub(crate) fn execute(
    feature: &PlannedFeature,
    plan: &EpisodePlan,
) -> Result<Vec<Chunk>, LeRobotError> {
    match feature {
        PlannedFeature::Scalar { key, feature } => {
            let timelines =
                std::iter::once((*plan.timeline.name(), plan.time_column.clone())).collect();
            Ok(load_scalar(key, feature, &timelines, &plan.parquet_data)?.collect())
        }
        PlannedFeature::Image { key } => {
            Ok(load_episode_images(key, &plan.timeline, &plan.parquet_data)?.collect())
        }
        PlannedFeature::DepthImage { key } => {
            Ok(load_episode_depth_images(key, &plan.timeline, &plan.parquet_data)?.collect())
        }
        PlannedFeature::Text { entity, rows } => {
            Ok(vec![build_text_chunk(entity, rows, &plan.timeline)?])
        }
        PlannedFeature::Video { entity, video } => match video {
            PlannedVideo::Asset { file } => {
                let contents = std::fs::read(file).map_err(|err| LeRobotError::io(err, file))?;
                build_video_asset_chunks(entity, contents, &plan.timeline, plan.time_column.clone())
            }
            PlannedVideo::Stream {
                bytes,
                from_ts,
                to_ts,
            } => build_video_stream_chunks(
                entity,
                bytes,
                *from_ts,
                *to_ts,
                &plan.timeline,
                &plan.time_column,
            ),
        },
    }
}
