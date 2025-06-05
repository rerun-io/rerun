use crate::ViewerContext;

/// Convert a video timestamp from component to a video time.
pub fn video_timestamp_component_to_video_time(
    ctx: &ViewerContext<'_>,
    video_timestamp: re_types::components::VideoTimestamp,
    timescale: Option<re_video::Timescale>,
) -> re_video::Time {
    if let Some(timescale) = timescale {
        re_video::Time::from_nanos(video_timestamp.as_nanos(), timescale)
    } else {
        // If there's no timescale, assume that timestamps are frames and use our currently set fps.
        let fps = ctx.rec_cfg.time_ctrl.read().fps().unwrap_or(1.0);
        re_video::Time((video_timestamp.0.0 as f64 * fps as f64) as i64)
    }
}
