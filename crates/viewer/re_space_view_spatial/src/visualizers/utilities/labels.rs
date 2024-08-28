#![allow(clippy::too_many_arguments)]

use std::iter;

use itertools::{izip, Either};

use re_entity_db::InstancePathHash;
use re_log_types::{EntityPath, Instance};
use re_viewer_context::ResolvedAnnotationInfos;

use crate::visualizers::entity_iterator::clamped_or;

#[derive(Clone)]
pub enum UiLabelTarget {
    /// Labels a given rect (in scene coordinates)
    Rect(egui::Rect),

    /// Labels a given point (in scene coordinates)
    Point2D(egui::Pos2),

    /// A point in space.
    Position3D(glam::Vec3),
}

#[derive(Clone)]
pub struct UiLabel {
    pub text: String,
    pub color: egui::Color32,

    /// The shape/position being labeled.
    pub target: UiLabelTarget,

    /// What is hovered if this label is hovered.
    pub labeled_instance: InstancePathHash,
}

/// Maximum number of labels after which we stop displaying labels for that entity all together.
///
/// TODO(#4451): Hiding of labels should be configurable. This can be the heuristic for it.
pub const MAX_NUM_LABELS_PER_ENTITY: usize = 30;

/// Produces 3D ui labels from component data.
///
/// See [`process_labels()`] for further documentation.
pub fn process_labels_3d<'a>(
    entity_path: &'a EntityPath,
    num_instances: usize,
    overall_position: glam::Vec3,
    instance_positions: impl Iterator<Item = glam::Vec3> + 'a,
    labels: &'a [re_types::ArrowString],
    colors: &'a [egui::Color32],
    annotation_infos: &'a ResolvedAnnotationInfos,
    world_from_obj: glam::Affine3A,
) -> impl Iterator<Item = UiLabel> + 'a {
    process_labels(
        entity_path,
        num_instances,
        overall_position,
        instance_positions,
        labels,
        colors,
        annotation_infos,
        move |position| UiLabelTarget::Position3D(world_from_obj.transform_point3(position)),
    )
}

/// Produces 2D ui labels from component data.
///
/// See [`process_labels()`] for further documentation.
pub fn process_labels_2d<'a>(
    entity_path: &'a EntityPath,
    num_instances: usize,
    overall_position: glam::Vec2,
    instance_positions: impl Iterator<Item = glam::Vec2> + 'a,
    labels: &'a [re_types::ArrowString],
    colors: &'a [egui::Color32],
    annotation_infos: &'a ResolvedAnnotationInfos,
    world_from_obj: glam::Affine3A,
) -> impl Iterator<Item = UiLabel> + 'a {
    process_labels(
        entity_path,
        num_instances,
        overall_position,
        instance_positions,
        labels,
        colors,
        annotation_infos,
        move |position| {
            let point = world_from_obj.transform_point3(position.extend(0.0));
            UiLabelTarget::Point2D(egui::pos2(point.x, point.y))
        },
    )
}

/// Produces ui labels from component data, allowing the caller to produce [`UiLabelTarget`]s
/// as they see fit.
///
/// Implements policy for displaying a single label vs. per-instance labels, or hiding labels.
///
/// * `num_instances` should be equal to the length of `instance_positions`.
/// * `overall_position` is the position where a single shared label will be displayed if it is.
///   This is typically the center of the bounding box of the entity.
/// * The number of per-instance labels actually drawn is the minimum of the lengths of
///   `instance_positions` and `labels`.
pub fn process_labels<'a, P: 'a>(
    entity_path: &'a EntityPath,
    num_instances: usize,
    overall_position: P,
    instance_positions: impl Iterator<Item = P> + 'a,
    labels: &'a [re_types::ArrowString],
    colors: &'a [egui::Color32],
    annotation_infos: &'a ResolvedAnnotationInfos,
    target_from_position: impl Fn(P) -> UiLabelTarget + 'a,
) -> impl Iterator<Item = UiLabel> + 'a {
    if labels.len() > 1 && num_instances > MAX_NUM_LABELS_PER_ENTITY {
        // Too many labels. Don't draw them.
        return Either::Left(iter::empty());
    }

    // If there's many instances but only a single label, place the single label at the
    // overall_position (which is usually a bounding box center).
    // TODO(andreas): A smoothed over time (+ discontinuity detection) bounding box would be great.
    let label_positions = if labels.len() == 1 && num_instances > 1 {
        Either::Left(std::iter::once(overall_position))
    } else {
        Either::Right(instance_positions)
    };

    let labels = izip!(
        annotation_infos.iter(),
        labels.iter().map(Some).chain(std::iter::repeat(None))
    )
    .map(|(annotation_info, label)| annotation_info.label(label.map(|l| l.as_str())));

    let colors = clamped_or(colors, &egui::Color32::WHITE);

    Either::Right(
        itertools::izip!(label_positions, labels, colors)
            .enumerate()
            .filter_map(move |(i, (position, label, color))| {
                label.map(|label| UiLabel {
                    text: label,
                    color: *color,
                    target: target_from_position(position),
                    labeled_instance: InstancePathHash::instance(
                        entity_path,
                        Instance::from(i as u64),
                    ),
                })
            }),
    )
}
