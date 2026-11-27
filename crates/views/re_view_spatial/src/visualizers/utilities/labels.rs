use std::iter;

use egui::Color32;
use itertools::Either;
use re_entity_db::InstancePathHash;
use re_log_types::{EntityPath, Instance};
use re_sdk_types::blueprint::components::VisualizerInstructionId;
use re_view::clamped_or;

use crate::SpaceKind;

#[derive(Clone)]
pub enum UiLabelTarget {
    /// Labels a given rect (in scene coordinates)
    Rect(egui::Rect),

    /// Labels a given point (in scene coordinates)
    Point2D(egui::Pos2),

    /// A point in space.
    Position3D(glam::Vec3),
}

impl From<glam::Vec2> for UiLabelTarget {
    fn from(position: glam::Vec2) -> Self {
        Self::Point2D(egui::pos2(position.x, position.y))
    }
}

impl From<glam::Vec3> for UiLabelTarget {
    fn from(position: glam::Vec3) -> Self {
        Self::Position3D(position)
    }
}

impl From<egui::Rect> for UiLabelTarget {
    fn from(rect: egui::Rect) -> Self {
        Self::Rect(rect)
    }
}

impl UiLabelTarget {
    /// The 2D target must be in object coordinates; existing 3D targets are already in world space.
    pub fn into_3d(self, world_from_obj: glam::Affine3A) -> Self {
        let anchor2d = match self {
            Self::Rect(rect) => rect.center_bottom(),
            Self::Point2D(pos) => pos,
            Self::Position3D(_) => return self,
        };
        world_from_obj
            .transform_point3(glam::vec3(anchor2d.x, anchor2d.y, 0.0))
            .into()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum UiLabelStyle {
    Default,

    Color(egui::Color32),

    /// Style it like an error message
    Error,
}

impl From<egui::Color32> for UiLabelStyle {
    fn from(color: egui::Color32) -> Self {
        Self::Color(color)
    }
}

#[derive(Clone)]
pub struct UiLabel {
    pub text: String,

    pub style: UiLabelStyle,

    /// The shape/position being labeled.
    pub target: UiLabelTarget,

    /// What is hovered if this label is hovered.
    pub labeled_instance: InstancePathHash,

    /// The visualizer instruction that produced this label.
    pub visualizer_instruction: VisualizerInstructionId,
}

/// Inputs for [`process_labels_2d()`] and [`process_labels_3d()`], defining the labels of a single [batch].
///
/// * `P` is the type of the _positions_ of the labels, which might be 3D or 2D.
/// * `I` is the type of the iterator over positions.
///
/// [batch]: https://rerun.io/docs/log/data-model/batches
pub struct LabeledBatch<'a, P: 'a, I: Iterator<Item = P> + 'a> {
    pub entity_path: &'a EntityPath,

    pub visualizer_instruction: VisualizerInstructionId,

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
    pub labels: &'a [re_sdk_types::ArrowString],

    /// Colors from the batch to apply to the labels.
    ///
    /// Length 1 is treated as a color for the whole batch.
    pub colors: &'a [egui::Color32],

    /// The [`re_sdk_types::components::ShowLabels`] component value.
    ///
    /// If no value is available from the data, use the fallback
    /// registry to obtain it.
    pub show_labels: re_sdk_types::components::ShowLabels,
}

/// Produces 3D ui labels from component data.
///
/// See [`LabeledBatch`] for the shared-label and visibility rules.
pub fn process_labels_3d<'a>(
    batch: LabeledBatch<'a, glam::Vec3, impl Iterator<Item = glam::Vec3> + 'a>,
    world_from_obj: glam::Affine3A,
) -> impl Iterator<Item = UiLabel> + 'a {
    process_labels(batch, move |position| {
        world_from_obj.transform_point3(position).into()
    })
}

/// Positions and rectangles must be in object coordinates.
/// In 3D views, rectangle labels use the transformed bottom-center anchor.
///
/// See [`LabeledBatch`] for the shared-label and visibility rules.
pub fn process_labels_2d<'a, P: Into<UiLabelTarget> + 'a>(
    batch: LabeledBatch<'a, P, impl Iterator<Item = P> + 'a>,
    world_from_obj: glam::Affine3A,
    space_kind: SpaceKind,
) -> impl Iterator<Item = UiLabel> + 'a {
    process_labels(batch, move |position| {
        let target = position.into();
        if space_kind == SpaceKind::ThreeD {
            return target.into_3d(world_from_obj);
        }

        let transform_point = |position: egui::Pos2| {
            let point = world_from_obj.transform_point3(glam::vec3(position.x, position.y, 0.0));
            egui::pos2(point.x, point.y)
        };
        match target {
            UiLabelTarget::Point2D(position) => UiLabelTarget::Point2D(transform_point(position)),
            UiLabelTarget::Rect(rect) => {
                let corners = [
                    rect.left_top(),
                    rect.right_top(),
                    rect.left_bottom(),
                    rect.right_bottom(),
                ]
                .map(transform_point);
                egui::Rect::from_points(&corners).into()
            }
            UiLabelTarget::Position3D(_) => target,
        }
    })
}

/// Produces ui labels from component data, allowing the caller to produce [`UiLabelTarget`]s
/// as they see fit.
///
/// Implements policy for displaying a single label vs. per-instance labels, or hiding labels.
fn process_labels<'a, P: 'a>(
    batch: LabeledBatch<'a, P, impl Iterator<Item = P> + 'a>,
    target_from_position: impl Fn(P) -> UiLabelTarget + 'a,
) -> impl Iterator<Item = UiLabel> + 'a {
    let LabeledBatch {
        entity_path,
        visualizer_instruction,
        num_instances,
        overall_position,
        instance_positions,
        labels,
        colors,
        show_labels,
    } = batch;
    let show_labels = bool::from(show_labels.0);

    if !show_labels {
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

    let labels = labels.iter().map(|label| Some(label.to_string()));
    let colors = clamped_or(colors, &Color32::PLACEHOLDER);

    Either::Right(
        itertools::izip!(label_positions, labels, colors)
            .enumerate()
            .filter_map(move |(i, (position, label, color))| {
                let label = label?;
                (!label.is_empty()).then(|| UiLabel {
                    text: label,
                    style: if *color == Color32::PLACEHOLDER {
                        UiLabelStyle::Default
                    } else {
                        UiLabelStyle::Color(*color)
                    },
                    target: target_from_position(position),
                    labeled_instance: InstancePathHash::instance(
                        entity_path,
                        Instance::from(i as u64),
                    ),
                    visualizer_instruction,
                })
            }),
    )
}
