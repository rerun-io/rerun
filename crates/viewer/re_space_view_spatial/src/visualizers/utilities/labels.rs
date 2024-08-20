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

/// Inputs for [`process_labels()`], defining the label(s) of a single [batch].
///
/// * `P` is the type of the _positions_ of the labels, which might be 3D or 2D.
/// * `I` is the type of the iterator over positions.
///
/// [batch]: https://rerun.io/docs/concepts/batches
pub struct LabeledBatch<'a, P: 'a, I: Iterator<Item = P> + 'a> {
    pub entity_path: &'a EntityPath,

    /// `num_instances` should be equal to the length of `instance_positions`.
    pub num_instances: usize,

    /// The position where a single shared label will be displayed if it is.
    /// This is typically the center of the bounding box of the entity.
    pub overall_position: P,

    /// Note: If we find a reason to make this data type usable more than once,
    /// replace this `Iterator` with `IntoIterator`.
    pub instance_positions: I,

    /// Label data from the batch.
    ///
    /// Length 1 is treated as a label for the whole batch.
    ///
    /// The number of per-instance labels actually drawn is the minimum of the lengths of
    /// `instance_positions` and `labels`.
    pub labels: &'a [re_types::ArrowString],

    /// Colors from the batch to apply to the labels.
    ///
    /// Length 1 is treated as a color for the whole batch.
    pub colors: &'a [egui::Color32],

    pub annotation_infos: &'a ResolvedAnnotationInfos,
}

/// Maximum number of labels after which we stop displaying labels for that entity all together.
///
/// TODO(#4451): Hiding of labels should be configurable. This can be the heuristic for it.
pub const MAX_NUM_LABELS_PER_ENTITY: usize = 30;

/// Produces 3D ui labels from component data.
///
/// See [`process_labels()`] for further documentation.
pub fn process_labels_3d<'a>(
    batch: LabeledBatch<'a, glam::Vec3, impl Iterator<Item = glam::Vec3> + 'a>,
    world_from_obj: glam::Affine3A,
) -> impl Iterator<Item = UiLabel> + 'a {
    process_labels(batch, move |position| {
        UiLabelTarget::Position3D(world_from_obj.transform_point3(position))
    })
}

/// Produces 2D ui labels from component data.
///
/// See [`process_labels()`] for further documentation.
pub fn process_labels_2d<'a>(
    batch: LabeledBatch<'a, glam::Vec2, impl Iterator<Item = glam::Vec2> + 'a>,
    world_from_obj: glam::Affine3A,
) -> impl Iterator<Item = UiLabel> + 'a {
    process_labels(batch, move |position| {
        let point = world_from_obj.transform_point3(position.extend(0.0));
        UiLabelTarget::Point2D(egui::pos2(point.x, point.y))
    })
}

/// Produces ui labels from component data, allowing the caller to produce [`UiLabelTarget`]s
/// as they see fit.
///
/// Implements policy for displaying a single label vs. per-instance labels, or hiding labels.
pub fn process_labels<'a, P: 'a>(
    batch: LabeledBatch<'a, P, impl Iterator<Item = P> + 'a>,
    target_from_position: impl Fn(P) -> UiLabelTarget + 'a,
) -> impl Iterator<Item = UiLabel> + 'a {
    let LabeledBatch {
        entity_path,
        num_instances,
        overall_position,
        instance_positions,
        labels,
        colors,
        annotation_infos,
    } = batch;

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
