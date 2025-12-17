/// Helper for suffixing image frame IDs with `image_plane`.
///
/// This is required to match the Rerun model for named pinhole frames, where the image plane has its own frame ID
/// different from the pinhole frame. In ROS, both image and camera info share the same frame ID.
pub fn suffix_image_plane_frame_ids(frame_ids: impl IntoIterator<Item = String>) -> Vec<String> {
    frame_ids
        .into_iter()
        .map(|id| format!("{id}_image_plane"))
        .collect::<Vec<_>>()
}
