use itertools::izip;

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
/// Does nothing if there's no positions or no labels passed.
/// Otherwise, produces one label per position passed.
//
// TODO(cmc): remove
pub fn process_labels_3d<'a>(
    entity_path: &'a EntityPath,
    positions: impl Iterator<Item = glam::Vec3> + 'a,
    labels: &'a [re_types::components::Text],
    colors: &'a [egui::Color32],
    annotation_infos: &'a ResolvedAnnotationInfos,
    world_from_obj: glam::Affine3A,
) -> impl Iterator<Item = UiLabel> + 'a {
    let labels = izip!(
        annotation_infos.iter(),
        labels.iter().map(Some).chain(std::iter::repeat(None))
    )
    .map(|(annotation_info, label)| annotation_info.label(label.map(|l| l.as_str())));

    let colors = clamped_or(colors, &egui::Color32::WHITE);

    itertools::izip!(positions, labels, colors)
        .enumerate()
        .filter_map(move |(i, (point, label, color))| {
            label.map(|label| UiLabel {
                text: label,
                color: *color,
                target: UiLabelTarget::Position3D(world_from_obj.transform_point3(point)),
                labeled_instance: InstancePathHash::instance(entity_path, Instance::from(i as u64)),
            })
        })
}

/// Produces 3D ui labels from component data.
///
/// Does nothing if there's no positions or no labels passed.
/// Otherwise, produces one label per position passed.
pub fn process_labels_3d_2<'a>(
    entity_path: &'a EntityPath,
    positions: impl Iterator<Item = glam::Vec3> + 'a,
    labels: &'a [re_types::ArrowString],
    colors: &'a [egui::Color32],
    annotation_infos: &'a ResolvedAnnotationInfos,
    world_from_obj: glam::Affine3A,
) -> impl Iterator<Item = UiLabel> + 'a {
    let labels = izip!(
        annotation_infos.iter(),
        labels.iter().map(Some).chain(std::iter::repeat(None))
    )
    .map(|(annotation_info, label)| annotation_info.label(label.map(|l| l.as_str())));

    let colors = clamped_or(colors, &egui::Color32::WHITE);

    itertools::izip!(positions, labels, colors)
        .enumerate()
        .filter_map(move |(i, (point, label, color))| {
            label.map(|label| UiLabel {
                text: label,
                color: *color,
                target: UiLabelTarget::Position3D(world_from_obj.transform_point3(point)),
                labeled_instance: InstancePathHash::instance(entity_path, Instance::from(i as u64)),
            })
        })
}

/// Produces 2D ui labels from component data.
///
/// Does nothing if there's no positions or no labels passed.
/// Otherwise, produces one label per position passed.
pub fn process_labels_2d<'a>(
    entity_path: &'a EntityPath,
    positions: impl Iterator<Item = glam::Vec2> + 'a,
    labels: &'a [re_types::ArrowString],
    colors: &'a [egui::Color32],
    annotation_infos: &'a ResolvedAnnotationInfos,
    world_from_obj: glam::Affine3A,
) -> impl Iterator<Item = UiLabel> + 'a {
    let labels = izip!(
        annotation_infos.iter(),
        labels.iter().map(Some).chain(std::iter::repeat(None))
    )
    .map(|(annotation_info, label)| annotation_info.label(label.map(|l| l.as_str())));

    let colors = clamped_or(colors, &egui::Color32::WHITE);

    itertools::izip!(positions, labels, colors)
        .enumerate()
        .filter_map(move |(i, (point, label, color))| {
            label.map(|label| {
                let point = world_from_obj.transform_point3(glam::Vec3::new(point.x, point.y, 0.0));
                UiLabel {
                    text: label,
                    color: *color,
                    target: UiLabelTarget::Point2D(egui::pos2(point.x, point.y)),
                    labeled_instance: InstancePathHash::instance(
                        entity_path,
                        Instance::from(i as u64),
                    ),
                }
            })
        })
}
