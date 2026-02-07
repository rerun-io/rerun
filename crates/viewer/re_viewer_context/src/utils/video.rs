use crate::ViewerContext;

/// Convert a video timestamp from component to a video time.
pub fn video_timestamp_component_to_video_time(
    ctx: &ViewerContext<'_>,
    video_timestamp: re_sdk_types::components::VideoTimestamp,
    timescale: Option<re_video::Timescale>,
) -> re_video::Time {
    if let Some(timescale) = timescale {
        re_video::Time::from_nanos(video_timestamp.as_nanos(), timescale)
    } else {
        // If there's no timescale, assume that timestamps are frames and use our currently set fps.
        let fps = ctx.time_ctrl.fps().unwrap_or(1.0);
        re_video::Time((video_timestamp.0.0 as f64 * fps as f64) as i64)
    }
}

/// Extract video stream time from query time.
///
/// Video streams are handled like "infinite" videos both forward and backwards in time.
/// Therefore, the "time in the video" is whatever time we have on the timeline right now.
pub fn video_stream_time_from_query(query: &re_chunk_store::LatestAtQuery) -> re_video::Time {
    // Video streams are always using the timeline directly for their timestamps,
    // therefore, we can use the unaltered time for all timeline types.
    re_video::Time::new(query.at().as_i64())
}
