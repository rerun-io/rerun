use re_log_types::EntityPath;

/// The frame ids needed to import a ROS camera topic with spatial metadata.
pub struct SpatialCameraFrameIds {
    pub camera_frame_ids: Vec<String>,
    pub image_plane_frame_ids: Vec<String>,
}

/// Returns the spatial frame ids for a ROS camera topic, or `None` if any row is missing a
/// `header.frame_id` and the topic should be downgraded to plain 2D import.
pub fn spatial_camera_frame_ids_or_log_missing(
    topic: &str,
    entity_path: &EntityPath,
    data_kind: &str,
    fallback: &str,
    camera_frame_ids: Vec<String>,
) -> Option<SpatialCameraFrameIds> {
    if camera_frame_ids
        .iter()
        .any(|frame_id| frame_id.trim().is_empty())
    {
        re_log::warn_once!(
            "Ignoring spatial camera metadata for {data_kind} on topic {topic:?} at entity {entity_path:?}: at least one message had a missing ROS `header.frame_id`. {fallback}"
        );
        None
    } else {
        let image_plane_frame_ids = camera_frame_ids
            .iter()
            .map(|frame_id| format!("{frame_id}_image_plane"))
            .collect();

        Some(SpatialCameraFrameIds {
            camera_frame_ids,
            image_plane_frame_ids,
        })
    }
}
