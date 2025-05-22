mod video_frame_reference;
mod video_stream;

pub use video_frame_reference::VideoFrameReferenceVisualizer;
pub use video_stream::VideoStreamVisualizer;

/// Identify a video stream for a given video.
fn video_stream_id(
    entity_path: &re_log_types::EntityPath,
    view_id: re_viewer_context::ViewId,
    visualizer_name: re_viewer_context::ViewSystemIdentifier,
) -> re_renderer::video::VideoPlayerStreamId {
    re_renderer::video::VideoPlayerStreamId(
        re_log_types::hash::Hash64::hash((entity_path.hash(), view_id, visualizer_name)).hash64(),
    )
}
