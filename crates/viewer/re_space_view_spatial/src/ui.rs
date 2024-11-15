use egui::{epaint::util::OrderedFloat, text::TextWrapping, NumExt as _, WidgetText};

use re_format::format_f32;
use re_math::BoundingBox;
use re_space_view::ScreenshotMode;
use re_types::{
    archetypes::Pinhole, blueprint::components::VisualBounds2D, components::ViewCoordinates,
    image::ImageKind,
};
use re_ui::UiExt as _;
use re_viewer_context::{
    HoverHighlight, SelectionHighlight, SpaceViewHighlights, SpaceViewState, ViewerContext,
};

use crate::{
    eye::EyeMode,
    pickable_textured_rect::PickableRectSourceData,
    picking::{PickableUiRect, PickingResult},
    scene_bounding_boxes::SceneBoundingBoxes,
    view_kind::SpatialSpaceViewKind,
    visualizers::{SpatialViewVisualizerData, UiLabel, UiLabelStyle, UiLabelTarget},
};

use super::{eye::Eye, ui_3d::View3DState};

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

/// TODO(andreas): Should turn this "inside out" - [`SpatialSpaceViewState`] should be used by [`View3DState`], not the other way round.
#[derive(Clone, Default)]
pub struct SpatialSpaceViewState {
    pub bounding_boxes: SceneBoundingBoxes,

    /// Number of images & depth images processed last frame.
    pub num_non_segmentation_images_last_frame: usize,

    /// Last frame's picking result.
    pub previous_picking_result: Option<PickingResult>,

    pub(super) state_3d: View3DState,

    /// Pinhole component logged at the origin if any.
    pub pinhole_at_origin: Option<Pinhole>,

    pub visual_bounds_2d: Option<VisualBounds2D>,
}

impl SpaceViewState for SpatialSpaceViewState {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl SpatialSpaceViewState {
    /// Updates the state with statistics from the latest system outputs.
    pub fn update_frame_statistics(
        &mut self,
        ui: &egui::Ui,
        system_output: &re_viewer_context::SystemExecutionOutput,
        space_kind: SpatialSpaceViewKind,
    ) {
        re_tracing::profile_function!();

        self.bounding_boxes
            .update(ui, &system_output.view_systems, space_kind);

        let view_systems = &system_output.view_systems;
        self.num_non_segmentation_images_last_frame = view_systems
            .iter_visualizer_data::<SpatialViewVisualizerData>()
            .flat_map(|data| {
                data.pickable_rects.iter().map(|pickable_rect| {
                    if let PickableRectSourceData::Image { image, .. } = &pickable_rect.source_data
                    {
                        (image.kind != ImageKind::Segmentation) as usize
                    } else {
                        0
                    }
                })
            })
            .sum();
    }

    pub fn bounding_box_ui(&mut self, ui: &mut egui::Ui, spatial_kind: SpatialSpaceViewKind) {
        ui.grid_left_hand_label("Bounding box")
            .on_hover_text("The bounding box encompassing all Entities in the view right now");
        ui.vertical(|ui| {
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
            let BoundingBox { min, max } = self.bounding_boxes.current;
            ui.label(format!("x [{} - {}]", format_f32(min.x), format_f32(max.x),));
            ui.label(format!("y [{} - {}]", format_f32(min.y), format_f32(max.y),));
            if spatial_kind == SpatialSpaceViewKind::ThreeD {
                ui.label(format!("z [{} - {}]", format_f32(min.z), format_f32(max.z),));
            }
        });
        ui.end_row();
    }

    // Say the name out loud. It is fun!
    pub fn view_eye_ui(
        &mut self,
        ui: &mut egui::Ui,
        scene_view_coordinates: Option<ViewCoordinates>,
    ) {
        if ui
            .button("Reset")
            .on_hover_text(
                "Resets camera position & orientation.\nYou can also double-click the 3D view.",
            )
            .clicked()
        {
            self.bounding_boxes.smoothed = self.bounding_boxes.current;
            self.state_3d
                .reset_camera(&self.bounding_boxes, scene_view_coordinates);
        }

        {
            let mut spin = self.state_3d.spin();
            if ui
                .re_checkbox(&mut spin, "Spin")
                .on_hover_text("Spin camera around the orbit center")
                .changed()
            {
                self.state_3d.set_spin(spin);
            }
        }

        if let Some(eye) = &mut self.state_3d.view_eye {
            ui.selectable_toggle(|ui| {
                let mut mode = eye.mode();
                ui.selectable_value(&mut mode, EyeMode::FirstPerson, "First Person");
                ui.selectable_value(&mut mode, EyeMode::Orbital, "Orbital");
                eye.set_mode(mode);
            });
        }
    }
}

pub fn create_labels(
    mut labels: Vec<UiLabel>,
    ui_from_scene: egui::emath::RectTransform,
    eye3d: &Eye,
    parent_ui: &egui::Ui,
    highlights: &SpaceViewHighlights,
    spatial_kind: SpatialSpaceViewKind,
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
                // TODO(#1640): 2D labels are not visible in 3D for now.
                if spatial_kind == SpatialSpaceViewKind::ThreeD {
                    continue;
                }
                let rect_in_ui = ui_from_scene.transform_rect(rect);
                (
                    // Place the text centered below the rect
                    (rect_in_ui.width() - 4.0).at_least(60.0),
                    rect_in_ui.center_bottom() + egui::vec2(0.0, 3.0),
                )
            }
            UiLabelTarget::Point2D(pos) => {
                // TODO(#1640): 2D labels are not visible in 3D for now.
                if spatial_kind == SpatialSpaceViewKind::ThreeD {
                    continue;
                }
                let pos_in_ui = ui_from_scene.transform_pos(pos);
                (f32::INFINITY, pos_in_ui + egui::vec2(0.0, 3.0))
            }
            UiLabelTarget::Position3D(pos) => {
                // TODO(#1640): 3D labels are not visible in 2D for now.
                if spatial_kind == SpatialSpaceViewKind::TwoD {
                    continue;
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
            UiLabelStyle::Color(color) => color,
            UiLabelStyle::Error => parent_ui.style().visuals.strong_text_color(),
        };
        let format = egui::TextFormat::simple(font_id, text_color);

        let galley = parent_ui.fonts(|fonts| {
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

        let text_rect = egui::Align2::CENTER_TOP
            .anchor_rect(egui::Rect::from_min_size(text_anchor_pos, galley.size()));
        let bg_rect = text_rect.expand2(egui::vec2(4.0, 2.0));

        let highlight = highlights
            .entity_highlight(label.labeled_instance.entity_path_hash)
            .index_highlight(label.labeled_instance.instance);
        let background_color = match highlight.hover {
            HoverHighlight::None => match highlight.selection {
                SelectionHighlight::None => {
                    if is_error {
                        parent_ui.error_label_background_color()
                    } else {
                        parent_ui.style().visuals.panel_fill
                    }
                }
                SelectionHighlight::SiblingSelection => {
                    parent_ui.style().visuals.widgets.active.bg_fill
                }
                SelectionHighlight::Selection => parent_ui.style().visuals.widgets.active.bg_fill,
            },
            HoverHighlight::Hovered => parent_ui.style().visuals.widgets.hovered.bg_fill,
        };

        let rect_stroke = if is_error {
            egui::Stroke::new(1.0, parent_ui.style().visuals.error_fg_color)
        } else {
            egui::Stroke::NONE
        };

        label_shapes.push(
            egui::epaint::RectShape::new(bg_rect.expand(4.0), 4.0, background_color, rect_stroke)
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

pub fn paint_loading_spinners(
    ui: &egui::Ui,
    ui_from_scene: egui::emath::RectTransform,
    eye3d: &Eye,
    visualizers: &re_viewer_context::VisualizerCollection,
) {
    use glam::Vec3Swizzles as _;
    use glam::Vec4Swizzles as _;

    let ui_from_world_3d = eye3d.ui_from_world(*ui_from_scene.to());

    for data in visualizers.iter_visualizer_data::<SpatialViewVisualizerData>() {
        for &crate::visualizers::LoadingSpinner {
            center,
            half_extent_u,
            half_extent_v,
        } in &data.loading_spinners
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

            egui::Spinner::new().paint_at(ui, rect);
        }
    }
}

pub fn screenshot_context_menu(
    _ctx: &ViewerContext<'_>,
    _response: &egui::Response,
) -> Option<ScreenshotMode> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        if _ctx.app_options.experimental_space_view_screenshots {
            let mut take_screenshot = None;
            _response.context_menu(|ui| {
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                if ui.button("Save screenshot to disk").clicked() {
                    take_screenshot = Some(ScreenshotMode::SaveAndCopyToClipboard);
                    ui.close_menu();
                } else if ui.button("Copy screenshot to clipboard").clicked() {
                    take_screenshot = Some(ScreenshotMode::CopyToClipboard);
                    ui.close_menu();
                }
            });
            take_screenshot
        } else {
            None
        }
    }
    #[cfg(target_arch = "wasm32")]
    {
        None
    }
}

pub fn format_vector(v: glam::Vec3) -> String {
    use glam::Vec3;

    if v == Vec3::X {
        "+X".to_owned()
    } else if v == -Vec3::X {
        "-X".to_owned()
    } else if v == Vec3::Y {
        "+Y".to_owned()
    } else if v == -Vec3::Y {
        "-Y".to_owned()
    } else if v == Vec3::Z {
        "+Z".to_owned()
    } else if v == -Vec3::Z {
        "-Z".to_owned()
    } else {
        format!("[{:.02}, {:.02}, {:.02}]", v.x, v.y, v.z)
    }
}
