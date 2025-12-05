use ahash::HashMap;
use re_sdk_types::ComponentIdentifier;
use re_sdk_types::components::Color;
use re_viewer_context::{Annotations, QueryContext, ResolvedAnnotationInfos, typed_fallback_for};

use crate::clamped_or_nothing;

/// Process [`Color`] components using annotations and default colors.
pub fn process_color_slice<'a>(
    ctx: &QueryContext<'_>,
    component: ComponentIdentifier,
    num_instances: usize,
    annotation_infos: &'a ResolvedAnnotationInfos,
    colors: &'a [Color],
) -> Vec<egui::Color32> {
    re_tracing::profile_function_if!(10_000 < num_instances);

    if let Some(last_color) = colors.last() {
        // If we have colors we can ignore the annotation infos/contexts.

        if colors.len() == num_instances {
            // Common happy path
            colors.iter().map(|c| egui::Color32::from(*c)).collect()
        } else if colors.len() == 1 {
            // Common happy path
            vec![egui::Color32::from(*last_color); num_instances]
        } else {
            let colors = clamped_or_nothing(colors, num_instances);
            colors.map(|c| egui::Color32::from(*c)).collect()
        }
    } else {
        match annotation_infos {
            ResolvedAnnotationInfos::Same(count, annotation_info) => {
                re_tracing::profile_scope!("no colors, same annotation");
                let color = annotation_info
                    .color()
                    .unwrap_or_else(|| typed_fallback_for::<Color>(ctx, component).into());
                vec![color; *count]
            }
            ResolvedAnnotationInfos::Many(annotation_info) => {
                re_tracing::profile_scope!("no-colors, many annotations");
                let fallback = typed_fallback_for::<Color>(ctx, component).into();
                annotation_info
                    .iter()
                    .map(|annotation_info| annotation_info.color().unwrap_or(fallback))
                    .collect()
            }
        }
    }
}

pub type Keypoints = HashMap<
    (re_sdk_types::components::ClassId, i64),
    HashMap<re_sdk_types::datatypes::KeypointId, glam::Vec3>,
>;

/// Resolves all annotations and keypoints for the given entity view.
pub fn process_annotation_and_keypoint_slices(
    latest_at: re_log_types::TimeInt,
    num_instances: usize,
    positions: impl Iterator<Item = glam::Vec3>,
    keypoint_ids: &[re_sdk_types::components::KeypointId],
    class_ids: &[re_sdk_types::components::ClassId],
    annotations: &Annotations,
) -> (ResolvedAnnotationInfos, Keypoints) {
    re_tracing::profile_function!();

    let mut keypoints: Keypoints = HashMap::default();

    // No need to process annotations if we don't have class-ids
    if class_ids.is_empty() {
        let resolved_annotation = annotations
            .resolved_class_description(None)
            .annotation_info();

        return (
            ResolvedAnnotationInfos::Same(num_instances, resolved_annotation),
            keypoints,
        );
    }

    let class_ids = clamped_or_nothing(class_ids, num_instances);

    if keypoint_ids.is_empty() {
        let annotation_info = class_ids
            .map(|&class_id| {
                let class_description = annotations.resolved_class_description(Some(class_id));
                class_description.annotation_info()
            })
            .collect();

        (
            ResolvedAnnotationInfos::Many(annotation_info),
            Default::default(),
        )
    } else {
        let keypoint_ids = clamped_or_nothing(keypoint_ids, num_instances);
        let annotation_info = itertools::izip!(positions, keypoint_ids, class_ids)
            .map(|(position, keypoint_id, &class_id)| {
                let class_description = annotations.resolved_class_description(Some(class_id));

                keypoints
                    .entry((class_id, latest_at.as_i64()))
                    .or_default()
                    .insert(keypoint_id.0, position);
                class_description.annotation_info_with_keypoint(keypoint_id.0)
            })
            .collect();

        (ResolvedAnnotationInfos::Many(annotation_info), keypoints)
    }
}

/// Resolves all annotations for the given entity view.
pub fn process_annotation_slices(
    latest_at: re_log_types::TimeInt,
    num_instances: usize,
    class_ids: &[re_sdk_types::components::ClassId],
    annotations: &Annotations,
) -> ResolvedAnnotationInfos {
    let (annotations, _keypoints) = process_annotation_and_keypoint_slices(
        latest_at,
        num_instances,
        std::iter::empty(), // positions are only needed for keypoint lookup
        &[],
        class_ids,
        annotations,
    );

    annotations
}
