use ahash::HashMap;
use re_sdk_types::ComponentIdentifier;
use re_sdk_types::components::Color;
use re_viewer_context::{QueryContext, typed_fallback_for};

use crate::{clamped_or, clamped_or_nothing};

/// Applies instance clamping and fallback semantics to queried [`Color`] components.
pub fn process_color_slice(
    ctx: &QueryContext<'_>,
    component: ComponentIdentifier,
    num_instances: usize,
    colors: &[Color],
) -> Vec<egui::Color32> {
    re_tracing::profile_function_if!(10_000 < num_instances);

    if let Some(last_color) = colors.last() {
        if colors.len() == num_instances {
            colors.iter().map(|c| egui::Color32::from(*c)).collect()
        } else if colors.len() == 1 {
            vec![egui::Color32::from(*last_color); num_instances]
        } else {
            clamped_or_nothing(colors, num_instances)
                .map(|c| egui::Color32::from(*c))
                .collect()
        }
    } else {
        vec![typed_fallback_for::<Color>(ctx, component).into(); num_instances]
    }
}

pub type Keypoints = HashMap<
    (re_sdk_types::components::ClassId, i64),
    HashMap<re_sdk_types::encodings::KeypointId, glam::Vec3>,
>;

/// Collects keypoint positions for drawing connections.
pub fn process_keypoint_slices(
    latest_at: re_log_types::TimeInt,
    num_positions: usize,
    positions: impl Iterator<Item = glam::Vec3>,
    keypoint_ids: &[re_sdk_types::components::KeypointId],
    class_ids: &[re_sdk_types::components::ClassId],
) -> Keypoints {
    re_tracing::profile_function_if!(100_000 < num_positions);

    let mut keypoints: Keypoints = HashMap::default();
    let fallback_class_id = 0.into();
    let class_ids = clamped_or(class_ids, &fallback_class_id);
    let keypoint_ids = clamped_or_nothing(keypoint_ids, num_positions);

    for (position, keypoint_id, &class_id) in itertools::izip!(positions, keypoint_ids, class_ids) {
        keypoints
            .entry((class_id, latest_at.as_i64()))
            .or_default()
            .insert(keypoint_id.0, position);
    }

    keypoints
}
