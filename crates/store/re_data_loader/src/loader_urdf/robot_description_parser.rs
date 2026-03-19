//! Utilities for parsing URDF XML from strings, like e.g. a ROS `robot_description` topic.

use re_chunk::EntityPath;
use re_log_types::TimePoint;

/// Parses URDF XML and returns the chunks emitted by Rerun's built-in URDF loader.
///
/// `include_joint_transforms` controls whether static joint transforms from the URDF
/// are emitted in addition to the robot geometry.
pub(crate) fn build_urdf_chunks_from_xml(
    urdf_xml: &str,
    entity_path_prefix: &Option<EntityPath>,
    timepoint: &TimePoint,
    include_joint_transforms: bool,
) -> anyhow::Result<Vec<re_chunk::Chunk>> {
    let robot = urdf_rs::read_from_string(urdf_xml)?;

    let mut chunks = Vec::new();

    super::emit_robot(
        &mut |chunk| chunks.push(chunk),
        robot,
        None,
        entity_path_prefix,
        timepoint,
        include_joint_transforms,
    )?;

    Ok(chunks)
}
