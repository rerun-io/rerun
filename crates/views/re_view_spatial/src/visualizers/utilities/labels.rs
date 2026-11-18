use std::iter;

use egui::Color32;
use itertools::Either;
use re_entity_db::InstancePathHash;
use re_log_types::{EntityPath, Instance};
use re_sdk_types::blueprint::components::VisualizerInstructionId;
use re_view::clamped_or;

#[derive(Clone, Debug)]
pub enum UiLabelTarget {
    /// Labels a given rect (in scene coordinates)
    Rect(egui::Rect),

    /// Labels a given point (in scene coordinates)
    Point2D(egui::Pos2),

    /// A point in space.
    Position3D(glam::Vec3),
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

/// Inputs for [`process_labels()`], defining the label(s) of a single [batch].
///
/// * `P` is the type of the _positions_ of the labels, which might be 3D or 2D.
/// * `I` is the type of the iterator over positions.
///
/// [batch]: https://rerun.io/docs/logging-and-ingestion/data-model/batches
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

/// Transforms a [`UiLabelTarget`] from object space into scene space, so a label
/// tracks the geometry it annotates when the entity has a transform
/// (e.g. a scale `Transform3D`).
///
/// `Rect` and `Point2D` targets are documented to be in scene coordinates, but
/// `Boxes2D` and `Ellipses2D` labels used to be passed through untransformed, so
/// e.g. a scale transform moved the boxes but not their labels
/// (rerun-io/rerun#9211).
///
/// A rect is transformed by mapping its four corners and taking the
/// axis-aligned bounding box of the result. This is exact for
/// scale/translation, and a conservative fit for rotations.
pub fn transform_label_target(
    world_from_obj: glam::Affine3A,
    target: UiLabelTarget,
) -> UiLabelTarget {
    match target {
        UiLabelTarget::Rect(rect) => {
            let corners = [
                rect.left_top(),
                rect.right_top(),
                rect.right_bottom(),
                rect.left_bottom(),
            ]
            .map(|corner| {
                let point = world_from_obj.transform_point3(glam::vec3(corner.x, corner.y, 0.0));
                egui::pos2(point.x, point.y)
            });
            UiLabelTarget::Rect(egui::Rect::from_points(&corners))
        }
        UiLabelTarget::Point2D(pos) => {
            let point = world_from_obj.transform_point3(glam::vec3(pos.x, pos.y, 0.0));
            UiLabelTarget::Point2D(egui::pos2(point.x, point.y))
        }
        UiLabelTarget::Position3D(pos) => {
            UiLabelTarget::Position3D(world_from_obj.transform_point3(pos))
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use re_sdk_types::ArrowString;

    fn labeled_batch<'a>(
        entity_path: &'a EntityPath,
        num_instances: usize,
        overall_position: UiLabelTarget,
        instance_positions: impl Iterator<Item = UiLabelTarget> + 'a,
        labels: &'a [ArrowString],
    ) -> LabeledBatch<'a, UiLabelTarget, impl Iterator<Item = UiLabelTarget> + 'a> {
        const COLORS: &[egui::Color32] = &[egui::Color32::RED];
        LabeledBatch {
            entity_path,
            visualizer_instruction: Default::default(),
            num_instances,
            overall_position,
            instance_positions,
            labels,
            colors: COLORS,
            show_labels: re_sdk_types::components::ShowLabels(true.into()),
        }
    }

    /// Regression test for <https://github.com/rerun-io/rerun/issues/9211>:
    /// a scale `Transform3D` on a 2D view must move box labels along with the boxes.
    ///
    /// Mirrors what `Boxes2DVisualizer` does: one shared label for several boxes,
    /// placed at the object-space bounding-box center, with per-instance `Rect`
    /// targets in object space.
    #[test]
    fn boxes2d_labels_follow_entity_transform() {
        let world_from_obj = glam::Affine3A::from_scale(glam::vec3(0.5, 0.5, 1.0));

        let entity_path = EntityPath::from("world/boxes");
        let labels = [ArrowString::from("in world/boxes")];
        let batch = labeled_batch(
            &entity_path,
            5,
            UiLabelTarget::Point2D(egui::pos2(512.0, 512.0)),
            (0..5).map(|i| {
                let min = i as f32 * 100.0;
                UiLabelTarget::Rect(egui::Rect::from_min_max(
                    egui::pos2(min, min),
                    egui::pos2(min + 50.0, min + 50.0),
                ))
            }),
            &labels,
        );

        // What `Boxes2DVisualizer` does: map label targets into scene space.
        let ui_labels: Vec<UiLabel> = process_labels(batch, move |target| {
            transform_label_target(world_from_obj, target)
        })
        .collect();

        assert_eq!(ui_labels.len(), 1, "one shared label for the whole batch");
        // The single shared label is placed at the overall position, which must
        // be in scene coordinates, i.e. halved by the 0.5 scale transform.
        assert!(
            matches!(ui_labels[0].target, UiLabelTarget::Point2D(pos)
                if pos == egui::pos2(256.0, 256.0)),
            "shared label should be at the scaled bounding-box center (256, 256), got {:?}",
            ui_labels[0].target,
        );
    }

    /// Same scenario as above, but with one label per box, so the `Rect`
    /// targets themselves (not just the shared overall position) must be
    /// transformed into scene space.
    #[test]
    fn boxes2d_rect_label_targets_follow_entity_transform() {
        let world_from_obj = glam::Affine3A::from_scale(glam::vec3(0.5, 0.5, 1.0));

        let entity_path = EntityPath::from("world/boxes");
        let labels = [
            ArrowString::from("box 0"),
            ArrowString::from("box 1"),
            ArrowString::from("box 2"),
        ];
        let batch = labeled_batch(
            &entity_path,
            3,
            UiLabelTarget::Point2D(egui::pos2(512.0, 512.0)),
            (0..3).map(|i| {
                let min = i as f32 * 100.0;
                UiLabelTarget::Rect(egui::Rect::from_min_max(
                    egui::pos2(min, min),
                    egui::pos2(min + 50.0, min + 50.0),
                ))
            }),
            &labels,
        );

        let ui_labels: Vec<UiLabel> = process_labels(batch, move |target| {
            transform_label_target(world_from_obj, target)
        })
        .collect();

        assert_eq!(ui_labels.len(), 3);
        for (i, ui_label) in ui_labels.iter().enumerate() {
            let min = i as f32 * 50.0;
            assert!(
                matches!(ui_label.target, UiLabelTarget::Rect(rect)
                if rect == egui::Rect::from_min_max(
                    egui::pos2(min, min),
                    egui::pos2(min + 25.0, min + 25.0),
                )),
                "label {i} should annotate the scaled box, got {:?}",
                ui_label.target,
            );
        }
    }

    #[test]
    fn transform_label_target_applies_scale_and_translation() {
        let world_from_obj = glam::Affine3A::from_translation(glam::vec3(100.0, 0.0, 0.0))
            * glam::Affine3A::from_scale(glam::vec3(2.0, 2.0, 1.0));

        assert!(
            matches!(
                transform_label_target(
                    world_from_obj,
                    UiLabelTarget::Rect(egui::Rect::from_min_max(
                        egui::pos2(10.0, 10.0),
                        egui::pos2(20.0, 30.0),
                    )),
                ),
                UiLabelTarget::Rect(rect)
                    if rect
                        == egui::Rect::from_min_max(
                            egui::pos2(120.0, 20.0),
                            egui::pos2(140.0, 60.0),
                        )
            ),
            "rect should be scaled and translated into scene space"
        );

        assert!(
            matches!(
                transform_label_target(world_from_obj, UiLabelTarget::Point2D(egui::pos2(
                    10.0, 20.0
                ))),
                UiLabelTarget::Point2D(pos) if pos == egui::pos2(120.0, 40.0)
            ),
            "point should be scaled and translated into scene space"
        );

        assert!(
            matches!(
                transform_label_target(
                    world_from_obj,
                    UiLabelTarget::Position3D(glam::vec3(10.0, 20.0, 30.0)),
                ),
                UiLabelTarget::Position3D(pos) if pos == glam::vec3(120.0, 40.0, 30.0)
            ),
            "3D point should be scaled and translated into scene space"
        );
    }

    #[test]
    fn transform_label_target_rotates_rect_to_bounding_box() {
        // 90-degree rotation about the origin: (x, y) -> (-y, x).
        let world_from_obj = glam::Affine3A::from_rotation_z(std::f32::consts::FRAC_PI_2);

        let target = transform_label_target(
            world_from_obj,
            UiLabelTarget::Rect(egui::Rect::from_min_max(
                egui::pos2(0.0, 0.0),
                egui::pos2(10.0, 20.0),
            )),
        );
        // The rotated rect is the axis-aligned bounding box of its corners.
        // `sin`/`cos` of 90 degrees are not exact in `f32`, so compare with
        // a small tolerance instead of exact equality.
        let UiLabelTarget::Rect(rect) = target else {
            panic!("expected a Rect target, got {target:?}");
        };
        assert!(
            (rect.min.x - -20.0).abs() < 1e-5,
            "min.x should be -20, got {rect:?}"
        );
        assert!(
            (rect.min.y - 0.0).abs() < 1e-5,
            "min.y should be 0, got {rect:?}"
        );
        assert!(
            (rect.max.x - 0.0).abs() < 1e-5,
            "max.x should be 0, got {rect:?}"
        );
        assert!(
            (rect.max.y - 10.0).abs() < 1e-5,
            "max.y should be 10, got {rect:?}"
        );
    }
}
