use egui::emath::OrderedFloat;
use egui::text::TextWrapping;
use egui::{NumExt as _, WidgetText};
use macaw::BoundingBox;
use re_format::format_f32;
use re_sdk_types::blueprint::archetypes::EyeControls3D;
use re_sdk_types::blueprint::components::VisualBounds2D;
use re_sdk_types::image::ImageKind;
use re_ui::UiExt as _;
use re_viewer_context::{
    HoverHighlight, ImageInfo, SelectionHighlight, ViewHighlights, ViewId, ViewState, ViewerContext,
};
use re_viewport_blueprint::ViewProperty;

use super::eye::Eye;
use super::ui_3d::View3DState;
use crate::Pinhole;
use crate::pickable_textured_rect::PickableRectSourceData;
use crate::picking::{PickableUiRect, PickingResult};
use crate::scene_bounding_boxes::SceneBoundingBoxes;
use crate::view_kind::SpatialViewKind;
use crate::visualizers::{SpatialViewVisualizerData, UiLabel, UiLabelStyle, UiLabelTarget};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum AutoSizeUnit {
    Auto,
    UiPoints,
    World,
}

impl From<AutoSizeUnit> for WidgetText {
    fn from(val: AutoSizeUnit) -> Self {
        match val {
            AutoSizeUnit::Auto => "Auto".into(),
            AutoSizeUnit::UiPoints => "UI points".into(),
            AutoSizeUnit::World => "Scene units".into(),
        }
    }
}

/// Number of images per image kind.
#[derive(Clone, Copy, Default)]
pub struct ImageCounts {
    pub segmentation: usize,
    pub color: usize,
    pub depth: usize,
}

/// TODO(andreas): Should turn this "inside out" - [`SpatialViewState`] should be used by `View3DState`, not the other way round.
#[derive(Clone, Default)]
pub struct SpatialViewState {
    pub bounding_boxes: SceneBoundingBoxes,

    /// Number of images per image kind processed last frame.
    pub image_counts_last_frame: ImageCounts,

    /// Last frame's picking result.
    pub previous_picking_result: Option<PickingResult>,

    pub state_3d: View3DState,

    /// Pinhole component logged at the origin if any.
    pub pinhole_at_origin: Option<Pinhole>,

    pub visual_bounds_2d: Option<VisualBounds2D>,
}

impl ViewState for SpatialViewState {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl SpatialViewState {
    /// Updates the state with statistics from the latest system outputs.
    pub fn update_frame_statistics(
        &mut self,
        ui: &egui::Ui,
        system_output: &re_viewer_context::SystemExecutionOutput,
        space_kind: SpatialViewKind,
    ) {
        re_tracing::profile_function!();

        self.bounding_boxes
            .update(ui, &system_output.view_systems, space_kind);

        let view_systems = &system_output.view_systems;

        // Reset the counts and start over.
        self.image_counts_last_frame = Default::default();

        for data in view_systems.iter_visualizer_data::<SpatialViewVisualizerData>() {
            for pickable_rect in &data.pickable_rects {
                match &pickable_rect.source_data {
                    PickableRectSourceData::Image {
                        image: ImageInfo { kind, .. },
                        ..
                    } => match kind {
                        ImageKind::Segmentation => self.image_counts_last_frame.segmentation += 1,
                        ImageKind::Color => self.image_counts_last_frame.color += 1,
                        ImageKind::Depth => self.image_counts_last_frame.depth += 1,
                    },
                    PickableRectSourceData::Video => {
                        self.image_counts_last_frame.color += 1;
                    }
                    PickableRectSourceData::Placeholder => {}
                }
            }
        }
    }

    pub fn bounding_box_ui(&self, ui: &mut egui::Ui, spatial_kind: SpatialViewKind) {
        ui.grid_left_hand_label("Bounding box")
            .on_hover_text("The bounding box encompassing all Entities in the view right now");
        ui.vertical(|ui| {
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
            let BoundingBox { min, max } = self.bounding_boxes.current;
            ui.label(format!("x [{} - {}]", format_f32(min.x), format_f32(max.x),));
            ui.label(format!("y [{} - {}]", format_f32(min.y), format_f32(max.y),));
            if spatial_kind == SpatialViewKind::ThreeD {
                ui.label(format!("z [{} - {}]", format_f32(min.z), format_f32(max.z),));
            }
        });
        ui.end_row();
    }

    // Say the name out loud. It is fun!
    pub fn view_eye_ui(&mut self, ui: &mut egui::Ui, ctx: &ViewerContext<'_>, view_id: ViewId) {
        let eye_property = ViewProperty::from_archetype::<EyeControls3D>(
            ctx.blueprint_db(),
            ctx.blueprint_query,
            view_id,
        );

        if ui
            .button("Reset")
            .on_hover_text(
                "Resets camera position & orientation.\nYou can also double-click the 3D view.",
            )
            .clicked()
        {
            self.bounding_boxes.smoothed = self.bounding_boxes.current;
            self.state_3d.reset_eye(ctx, &eye_property);
        }
    }

    pub fn fallback_opacity_for_image_kind(&self, kind: ImageKind) -> f32 {
        // If we have multiple images in the same view, they should not be fully opaque
        // if there is at least one image of the same kind with equal or lower draw order.
        //
        // Here we also assume that if the opacity is unchanged, neither is the draw order.
        //
        // By default, the draw order is (front to back):
        // * segmentation image
        // * color image
        // * depth image
        let counts = self.image_counts_last_frame;
        match kind {
            ImageKind::Segmentation => {
                if counts.color + counts.depth > 0 {
                    // Segmentation images should always be transparent if there was more than one image in the view,
                    // excluding other segmentation images.
                    0.5
                } else {
                    1.0
                }
            }
            ImageKind::Color => {
                if counts.depth > 0 {
                    0.5
                } else {
                    1.0
                }
            }
            // NOTE: Depth images do not support opacity
            ImageKind::Depth => 1.0,
        }
    }

    /// Accesser method for getting the entity, if any, that was tracked last time
    /// the eye was updated.
    pub fn last_tracked_entity(&self) -> Option<&re_log_types::EntityPath> {
        self.state_3d.eye_state.last_tracked_entity.as_ref()
    }
}

pub fn create_labels(
    mut labels: Vec<UiLabel>,
    ui_from_scene: egui::emath::RectTransform,
    eye3d: &Eye,
    parent_ui: &egui::Ui,
    highlights: &ViewHighlights,
    spatial_kind: SpatialViewKind,
) -> (Vec<egui::Shape>, Vec<PickableUiRect>) {
    re_tracing::profile_function!();

    let ui_from_world_3d = eye3d.ui_from_world(*ui_from_scene.to());

    // Closest last (painters algorithm)
    labels.sort_by_key(|label| {
        if let UiLabelTarget::Position3D(pos) = label.target {
            OrderedFloat::from(-ui_from_world_3d.project_point3(pos).z)
        } else {
            OrderedFloat::from(0.0)
        }
    });

    let mut label_shapes = Vec::with_capacity(labels.len() * 2);
    let mut ui_rects = Vec::with_capacity(labels.len());

    for label in labels {
        let (wrap_width, text_anchor_pos) = match label.target {
            UiLabelTarget::Rect(rect) => {
                if spatial_kind == SpatialViewKind::ThreeD {
                    continue; // TODO(#1640): 2D labels are not visible in 3D for now.
                }
                let rect_in_ui = ui_from_scene.transform_rect(rect);
                (
                    // Place the text centered below the rect
                    (rect_in_ui.width() - 4.0).at_least(60.0),
                    rect_in_ui.center_bottom(),
                )
            }
            UiLabelTarget::Point2D(pos) => {
                if spatial_kind == SpatialViewKind::ThreeD {
                    continue; // TODO(#1640): 2D labels are not visible in 3D for now.
                }
                let pos_in_ui = ui_from_scene.transform_pos(pos);
                (f32::INFINITY, pos_in_ui)
            }
            UiLabelTarget::Position3D(pos) => {
                if spatial_kind == SpatialViewKind::TwoD {
                    continue; // TODO(#1640): 3D labels are not visible in 2D for now.
                }
                let pos_in_ui = ui_from_world_3d * pos.extend(1.0);
                if pos_in_ui.w <= 0.0 {
                    continue; // behind camera
                }
                let pos_in_ui = pos_in_ui / pos_in_ui.w;
                (f32::INFINITY, egui::pos2(pos_in_ui.x, pos_in_ui.y))
            }
        };

        let font_id = egui::TextStyle::Body.resolve(parent_ui.style());
        let is_error = matches!(label.style, UiLabelStyle::Error);
        let text_color = match label.style {
            UiLabelStyle::Default => parent_ui.visuals().strong_text_color(),
            UiLabelStyle::Color(color) => color,
            UiLabelStyle::Error => parent_ui.style().visuals.strong_text_color(),
        };
        let format = egui::TextFormat::simple(font_id, text_color);

        let galley = parent_ui.fonts_mut(|fonts| {
            fonts.layout_job({
                egui::text::LayoutJob {
                    sections: vec![egui::text::LayoutSection {
                        leading_space: 0.0,
                        byte_range: 0..label.text.len(),
                        format,
                    }],
                    text: label.text.clone(),
                    wrap: TextWrapping {
                        max_width: wrap_width,
                        ..Default::default()
                    },
                    break_on_newline: true,
                    halign: egui::Align::Center,
                    ..Default::default()
                }
            })
        });

        let offset = egui::vec2(0.0, 5.0); // Add some margin
        let text_rect =
            egui::Align2::CENTER_TOP.anchor_size(text_anchor_pos + offset, galley.size());
        let bg_rect = text_rect.expand2(egui::vec2(2.0, 0.0));

        let highlight = highlights
            .entity_highlight(label.labeled_instance.entity_path_hash)
            .index_highlight(label.labeled_instance.instance);
        let background_color = match highlight.hover {
            HoverHighlight::None => match highlight.selection {
                SelectionHighlight::None => {
                    if is_error {
                        parent_ui.error_label_background_color()
                    } else {
                        parent_ui.style().visuals.widgets.inactive.bg_fill
                    }
                }
                SelectionHighlight::SiblingSelection => {
                    parent_ui.style().visuals.widgets.active.bg_fill
                }
                SelectionHighlight::Selection => parent_ui.style().visuals.widgets.active.bg_fill,
            },
            HoverHighlight::Hovered => parent_ui.style().visuals.widgets.hovered.bg_fill,
        };

        let background_color =
            background_color.gamma_multiply(parent_ui.tokens().spatial_label_bg_opacity);

        let rect_stroke = if is_error {
            egui::Stroke::new(1.0, parent_ui.style().visuals.error_fg_color)
        } else {
            egui::Stroke::NONE
        };

        label_shapes.push(
            egui::epaint::RectShape::new(
                bg_rect.expand(4.0),
                4.0,
                background_color,
                rect_stroke,
                egui::StrokeKind::Outside,
            )
            .into(),
        );
        label_shapes.push(egui::Shape::galley(
            text_rect.center_top(),
            galley,
            text_color,
        ));

        ui_rects.push(PickableUiRect {
            rect: ui_from_scene.inverse().transform_rect(bg_rect),
            instance_hash: label.labeled_instance,
        });
    }

    (label_shapes, ui_rects)
}

pub fn paint_loading_indicators(
    ui: &egui::Ui,
    ui_from_scene: egui::emath::RectTransform,
    eye3d: &Eye,
    visualizers: &re_viewer_context::VisualizerCollection,
) {
    use glam::{Vec3Swizzles as _, Vec4Swizzles as _};

    let ui_from_world_3d = eye3d.ui_from_world(*ui_from_scene.to());

    for data in visualizers.iter_visualizer_data::<SpatialViewVisualizerData>() {
        for &crate::visualizers::LoadingIndicator {
            center,
            half_extent_u,
            half_extent_v,
        } in &data.loading_indicators
        {
            // Transform to ui coordinates:
            let center_unprojected = ui_from_world_3d * center.extend(1.0);
            if center_unprojected.w < 0.0 {
                continue; // behind camera eye
            }
            let center_in_scene: glam::Vec2 = center_unprojected.xy() / center_unprojected.w;

            let mut radius_in_scene = f32::INFINITY;

            // Estimate the radius so we are unlikely to exceed the projected box:
            for radius_vec in [half_extent_u, -half_extent_u, half_extent_v, -half_extent_v] {
                let axis_radius = center_in_scene
                    .distance(ui_from_world_3d.project_point3(center + radius_vec).xy());
                radius_in_scene = radius_in_scene.min(axis_radius);
            }

            radius_in_scene *= 0.75; // Shrink a bit

            let max_radius = 0.5 * ui_from_scene.from().size().min_elem();
            radius_in_scene = radius_in_scene.min(max_radius);

            let rect = egui::Rect::from_center_size(
                egui::pos2(center_in_scene.x, center_in_scene.y),
                egui::Vec2::splat(2.0 * radius_in_scene),
            );

            let rect = ui_from_scene.transform_rect(rect);

            re_ui::loading_indicator::paint_loading_indicator_inside(
                ui,
                egui::Align2::CENTER_CENTER,
                rect,
            );
        }
    }
}
