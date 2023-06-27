use eframe::epaint::text::TextWrapping;
use egui::{NumExt, WidgetText};
use macaw::BoundingBox;

use nohash_hasher::IntSet;
use re_components::{Pinhole, Tensor, TensorDataMeaning};
use re_data_store::{EditableAutoValue, EntityPath};
use re_data_ui::{item_ui, DataUi};
use re_data_ui::{show_zoomed_image_region, show_zoomed_image_region_area_outline};
use re_format::format_f32;
use re_renderer::OutlineConfig;
use re_space_view::ScreenshotMode;
use re_viewer_context::{
    HoverHighlight, HoveredSpace, Item, SelectionHighlight, SpaceViewHighlights, SpaceViewId,
    SpaceViewState, TensorDecodeCache, TensorStatsCache, UiVerbosity, ViewerContext,
};

use super::{
    eye::Eye,
    scene::{PickingHitType, PickingResult},
    ui_2d::View2DState,
    ui_3d::View3DState,
};

use crate::scene::preferred_navigation_mode;
use crate::{
    scene::{PickableUiRect, SceneSpatial, UiLabel, UiLabelTarget},
    ui_2d::view_2d,
    ui_3d::{view_3d, SpaceSpecs},
};

/// Default auto point radius in UI points.
const AUTO_POINT_RADIUS: f32 = 1.5;

/// Default auto line radius in UI points.
const AUTO_LINE_RADIUS: f32 = 1.5;

/// Describes how the scene is navigated, determining if it is a 2D or 3D experience.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub enum SpatialNavigationMode {
    #[default]
    TwoD,
    ThreeD,
}

impl From<SpatialNavigationMode> for WidgetText {
    fn from(val: SpatialNavigationMode) -> Self {
        match val {
            SpatialNavigationMode::TwoD => "2D Pan & Zoom".into(),
            SpatialNavigationMode::ThreeD => "3D Camera".into(),
        }
    }
}

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

#[derive(Clone)]
pub struct SpatialSpaceViewState {
    /// How the scene is navigated.
    pub nav_mode: EditableAutoValue<SpatialNavigationMode>,

    /// Estimated bounding box of all data. Accumulated over every time data is displayed.
    ///
    /// Specify default explicitly, otherwise it will be a box at 0.0 after deserialization.
    pub scene_bbox_accum: BoundingBox,

    /// Estimated bounding box of all data for the last scene query.
    pub scene_bbox: BoundingBox,

    /// Estimated number of primitives last frame. Used to inform some heuristics.
    pub scene_num_primitives: usize,

    /// Last frame's picking result.
    pub previous_picking_result: Option<PickingResult>,

    pub(super) state_2d: View2DState,
    pub(super) state_3d: View3DState,

    /// Size of automatically sized objects. None if it wasn't configured.
    auto_size_config: re_renderer::AutoSizeConfig,
}

impl Default for SpatialSpaceViewState {
    fn default() -> Self {
        Self {
            nav_mode: EditableAutoValue::Auto(SpatialNavigationMode::ThreeD),
            scene_bbox_accum: BoundingBox::nothing(),
            scene_bbox: BoundingBox::nothing(),
            scene_num_primitives: 0,
            state_2d: Default::default(),
            state_3d: Default::default(),
            auto_size_config: re_renderer::AutoSizeConfig {
                point_radius: re_renderer::Size::AUTO, // let re_renderer decide
                line_radius: re_renderer::Size::AUTO,  // let re_renderer decide
            },
            previous_picking_result: None,
        }
    }
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
    pub fn auto_size_config(&self) -> re_renderer::AutoSizeConfig {
        let mut config = self.auto_size_config;
        if config.point_radius.is_auto() {
            config.point_radius = re_renderer::Size::new_points(AUTO_POINT_RADIUS);
        }
        if config.line_radius.is_auto() {
            config.line_radius = re_renderer::Size::new_points(AUTO_LINE_RADIUS);
        }
        config
    }

    pub fn update_object_property_heuristics(
        &self,
        ctx: &mut ViewerContext<'_>,
        entity_paths: &IntSet<EntityPath>,
        entity_properties: &mut re_data_store::EntityPropertyMap,
    ) {
        re_tracing::profile_function!();

        for entity_path in entity_paths {
            self.update_pinhole_property_heuristics(ctx, entity_path, entity_properties);
            self.update_depth_cloud_property_heuristics(ctx, entity_path, entity_properties);
        }
    }

    fn auto_size_world_heuristic(&self) -> f32 {
        if self.scene_bbox_accum.is_nothing() || self.scene_bbox_accum.is_nan() {
            return 0.01;
        }

        // Motivation: Size should be proportional to the scene extent, here covered by its diagonal
        let diagonal_length = (self.scene_bbox_accum.max - self.scene_bbox_accum.min).length();
        let heuristic0 = diagonal_length * 0.0025;

        // Motivation: A lot of times we look at the entire scene and expect to see everything on a flat screen with some gaps between.
        let size = self.scene_bbox_accum.size();
        let mut sorted_components = size.to_array();
        sorted_components.sort_by(|a, b| a.partial_cmp(b).unwrap());
        // Median is more robust against outlier in one direction (as such pretty poor still though)
        let median_extent = sorted_components[1];
        // sqrt would make more sense but using a smaller root works better in practice.
        let heuristic1 =
            (median_extent / (self.scene_num_primitives.at_least(1) as f32).powf(1.0 / 1.7)) * 0.25;

        heuristic0.min(heuristic1)
    }

    fn update_pinhole_property_heuristics(
        &self,
        ctx: &mut ViewerContext<'_>,
        entity_path: &EntityPath,
        entity_properties: &mut re_data_store::EntityPropertyMap,
    ) {
        let store = &ctx.store_db.entity_db.data_store;
        if store
            .query_latest_component::<Pinhole>(entity_path, &ctx.current_query())
            .is_some()
        {
            let mut properties = entity_properties.get(entity_path);
            if properties.pinhole_image_plane_distance.is_auto() {
                let scene_size = self.scene_bbox_accum.size().length();
                let default_image_plane_distance = if scene_size.is_finite() && scene_size > 0.0 {
                    scene_size * 0.05
                } else {
                    1.0
                };
                properties.pinhole_image_plane_distance =
                    EditableAutoValue::Auto(default_image_plane_distance);
                entity_properties.set(entity_path.clone(), properties);
            }
        }
    }

    fn update_depth_cloud_property_heuristics(
        &self,
        ctx: &mut ViewerContext<'_>,
        entity_path: &EntityPath,
        entity_properties: &mut re_data_store::EntityPropertyMap,
    ) -> Option<()> {
        let store = &ctx.store_db.entity_db.data_store;
        let tensor = store.query_latest_component::<Tensor>(entity_path, &ctx.current_query())?;

        let mut properties = entity_properties.get(entity_path);
        if properties.backproject_depth.is_auto() {
            properties.backproject_depth = EditableAutoValue::Auto(
                tensor.meaning == TensorDataMeaning::Depth
                    && *self.nav_mode.get() == SpatialNavigationMode::ThreeD,
            );
        }

        if tensor.meaning == TensorDataMeaning::Depth {
            if properties.depth_from_world_scale.is_auto() {
                let auto = tensor.meter.unwrap_or_else(|| {
                    if tensor.dtype().is_integer() {
                        1000.0
                    } else {
                        1.0
                    }
                });
                properties.depth_from_world_scale = EditableAutoValue::Auto(auto);
            }

            if properties.backproject_radius_scale.is_auto() {
                properties.backproject_radius_scale = EditableAutoValue::Auto(1.0);
            }

            entity_properties.set(entity_path.clone(), properties);
        }

        Some(())
    }

    pub fn selection_ui(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        space_origin: &EntityPath,
        space_view_id: SpaceViewId,
    ) {
        ctx.re_ui.selection_grid(ui, "spatial_settings_ui")
            .show(ui, |ui| {
            let auto_size_world = self.auto_size_world_heuristic();

            ctx.re_ui.grid_left_hand_label(ui, "Space origin")
                .on_hover_text("The origin is at the origin of this Entity. All transforms are relative to it");
            item_ui::entity_path_button(ctx,
                ui,
                Some(space_view_id),
                space_origin,
            );
            ui.end_row();

            ctx.re_ui.grid_left_hand_label(ui, "Default size");
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.push_id("points", |ui| {
                        size_ui(
                            ui,
                            2.0,
                            auto_size_world,
                            &mut self.auto_size_config.point_radius,
                        );
                    });
                    ui.label("Point radius")
                    .on_hover_text("Point radius used whenever not explicitly specified.");
                });
                ui.horizontal(|ui| {
                    ui.push_id("lines", |ui| {
                        size_ui(
                            ui,
                            1.5,
                            auto_size_world,
                            &mut self.auto_size_config.line_radius,
                        );
                        ui.label("Line radius")
                            .on_hover_text("Line radius used whenever not explicitly specified.");
                    });
                });
            });
            ui.end_row();

            ctx.re_ui.grid_left_hand_label(ui, "Camera")
                .on_hover_text("The virtual camera which controls what is shown on screen.");
            ui.vertical(|ui| {
                let mut nav_mode = *self.nav_mode.get();
                let mut changed = false;
                egui::ComboBox::from_id_source("nav_mode")
                    .selected_text(nav_mode)
                    .show_ui(ui, |ui| {
                        ui.style_mut().wrap = Some(false);
                        ui.set_min_width(64.0);

                        changed |= ui.selectable_value(
                            &mut nav_mode,
                            SpatialNavigationMode::TwoD,
                            SpatialNavigationMode::TwoD,
                        ).changed();

                        changed |= ui.selectable_value(
                            &mut nav_mode,
                            SpatialNavigationMode::ThreeD,
                            SpatialNavigationMode::ThreeD,
                        ).changed();
                    });
                    if changed {
                        self.nav_mode = EditableAutoValue::UserEdited(nav_mode);
                    }

                if *self.nav_mode.get() == SpatialNavigationMode::ThreeD {
                    if ui.button("Reset").on_hover_text(
                        "Resets camera position & orientation.\nYou can also double-click the 3D view.")
                        .clicked()
                    {
                        self.state_3d.reset_camera(&self.scene_bbox_accum);
                    }
                    ui.checkbox(&mut self.state_3d.spin, "Spin")
                        .on_hover_text("Spin camera around the orbit center.");
                }
            });
            ui.end_row();

            if *self.nav_mode.get() == SpatialNavigationMode::ThreeD {
                ctx.re_ui.grid_left_hand_label(ui, "Coordinates")
                    .on_hover_text("The world coordinate system used for this view.");
                ui.vertical(|ui|{
                    ui.label(format!("Up is {}", axis_name(self.state_3d.space_specs.up))).on_hover_ui(|ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 0.0;
                            ui.label("Set with ");
                            ui.code("rerun.log_view_coordinates");
                            ui.label(".");
                        });
                    });
                    ui.checkbox(&mut self.state_3d.show_axes, "Show origin axes").on_hover_text("Show X-Y-Z axes");
                    ui.checkbox(&mut self.state_3d.show_bbox, "Show bounding box").on_hover_text("Show the current scene bounding box");
                });
                ui.end_row();
            }

            ctx.re_ui.grid_left_hand_label(ui, "Bounding box")
                .on_hover_text("The bounding box encompassing all Entities in the view right now.");
            ui.vertical(|ui| {
                ui.style_mut().wrap = Some(false);
                let BoundingBox { min, max } = self.scene_bbox;
                ui.label(format!(
                    "x [{} - {}]",
                    format_f32(min.x),
                    format_f32(max.x),
                ));
                ui.label(format!(
                    "y [{} - {}]",
                    format_f32(min.y),
                    format_f32(max.y),
                ));
                if *self.nav_mode.get() == SpatialNavigationMode::ThreeD {
                    ui.label(format!(
                        "z [{} - {}]",
                        format_f32(min.z),
                        format_f32(max.z),
                    ));
                }
            });
            ui.end_row();
        });
    }

    pub fn view_spatial(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        scene: &mut SceneSpatial,
        space_origin: &EntityPath,
        space_view_id: SpaceViewId,
    ) {
        self.scene_bbox = scene.parts.calculate_bounding_box();
        if self.scene_bbox_accum.is_nothing() {
            self.scene_bbox_accum = self.scene_bbox;
        } else {
            self.scene_bbox_accum = self.scene_bbox_accum.union(self.scene_bbox);
        }

        if self.nav_mode.is_auto() {
            self.nav_mode = EditableAutoValue::Auto(preferred_navigation_mode(scene, space_origin));
        }
        self.scene_num_primitives = scene
            .context
            .num_3d_primitives
            .load(std::sync::atomic::Ordering::Relaxed);

        let store = &ctx.store_db.entity_db.data_store;
        match *self.nav_mode.get() {
            SpatialNavigationMode::ThreeD => {
                let coordinates = store.query_latest_component(space_origin, &ctx.current_query());
                self.state_3d.space_specs = SpaceSpecs::from_view_coordinates(coordinates);
                view_3d(ctx, ui, self, space_origin, space_view_id, scene);
            }
            SpatialNavigationMode::TwoD => {
                let scene_rect_accum = egui::Rect::from_min_max(
                    self.scene_bbox_accum.min.truncate().to_array().into(),
                    self.scene_bbox_accum.max.truncate().to_array().into(),
                );
                view_2d(
                    ctx,
                    ui,
                    self,
                    scene,
                    space_origin,
                    space_view_id,
                    scene_rect_accum,
                );
            }
        }
    }

    pub fn help_text(&self, re_ui: &re_ui::ReUi) -> egui::WidgetText {
        match *self.nav_mode.get() {
            SpatialNavigationMode::TwoD => super::ui_2d::help_text(re_ui),
            SpatialNavigationMode::ThreeD => super::ui_3d::help_text(re_ui),
        }
    }
}

fn size_ui(
    ui: &mut egui::Ui,
    default_size_points: f32,
    default_size_world: f32,
    size: &mut re_renderer::Size,
) {
    use re_renderer::Size;

    let mut mode = if size.is_auto() {
        AutoSizeUnit::Auto
    } else if size.points().is_some() {
        AutoSizeUnit::UiPoints
    } else {
        AutoSizeUnit::World
    };

    let mode_before = mode;
    egui::ComboBox::from_id_source("auto_size_mode")
        .selected_text(mode)
        .show_ui(ui, |ui| {
            ui.style_mut().wrap = Some(false);
            ui.set_min_width(64.0);

            ui.selectable_value(&mut mode, AutoSizeUnit::Auto, AutoSizeUnit::Auto)
                .on_hover_text("Determine automatically.");
            ui.selectable_value(&mut mode, AutoSizeUnit::UiPoints, AutoSizeUnit::UiPoints)
                .on_hover_text("Manual in UI points.");
            ui.selectable_value(&mut mode, AutoSizeUnit::World, AutoSizeUnit::World)
                .on_hover_text("Manual in scene units.");
        });
    if mode != mode_before {
        *size = match mode {
            AutoSizeUnit::Auto => Size::AUTO,
            AutoSizeUnit::UiPoints => Size::new_points(default_size_points),
            AutoSizeUnit::World => Size::new_scene(default_size_world),
        };
    }

    if mode != AutoSizeUnit::Auto {
        let mut displayed_size = size.0.abs();
        let (drag_speed, clamp_range) = if mode == AutoSizeUnit::UiPoints {
            (0.1, 0.1..=250.0)
        } else {
            (0.01 * displayed_size, 0.0001..=f32::INFINITY)
        };
        if ui
            .add(
                egui::DragValue::new(&mut displayed_size)
                    .speed(drag_speed)
                    .clamp_range(clamp_range)
                    .max_decimals(4),
            )
            .changed()
        {
            *size = match mode {
                AutoSizeUnit::Auto => unreachable!(),
                AutoSizeUnit::UiPoints => Size::new_points(displayed_size),
                AutoSizeUnit::World => Size::new_scene(displayed_size),
            };
        }
    }
}

fn axis_name(axis: Option<glam::Vec3>) -> String {
    if let Some(axis) = axis {
        if axis == glam::Vec3::X {
            "+X".to_owned()
        } else if axis == -glam::Vec3::X {
            "-X".to_owned()
        } else if axis == glam::Vec3::Y {
            "+Y".to_owned()
        } else if axis == -glam::Vec3::Y {
            "-Y".to_owned()
        } else if axis == glam::Vec3::Z {
            "+Z".to_owned()
        } else if axis == -glam::Vec3::Z {
            "-Z".to_owned()
        } else if axis != glam::Vec3::ZERO {
            format!("Up is [{:.3} {:.3} {:.3}]", axis.x, axis.y, axis.z)
        } else {
            "—".to_owned()
        }
    } else {
        "—".to_owned()
    }
}

pub fn create_labels(
    labels: &[UiLabel],
    ui_from_canvas: egui::emath::RectTransform,
    eye3d: &Eye,
    parent_ui: &mut egui::Ui,
    highlights: &SpaceViewHighlights,
    nav_mode: SpatialNavigationMode,
) -> (Vec<egui::Shape>, Vec<PickableUiRect>) {
    re_tracing::profile_function!();

    let mut label_shapes = Vec::with_capacity(labels.len() * 2);
    let mut ui_rects = Vec::with_capacity(labels.len());

    let ui_from_world_3d = eye3d.ui_from_world(*ui_from_canvas.to());

    for label in labels {
        let (wrap_width, text_anchor_pos) = match label.target {
            UiLabelTarget::Rect(rect) => {
                // TODO(#1640): 2D labels are not visible in 3D for now.
                if nav_mode == SpatialNavigationMode::ThreeD {
                    continue;
                }
                let rect_in_ui = ui_from_canvas.transform_rect(rect);
                (
                    // Place the text centered below the rect
                    (rect_in_ui.width() - 4.0).at_least(60.0),
                    rect_in_ui.center_bottom() + egui::vec2(0.0, 3.0),
                )
            }
            UiLabelTarget::Point2D(pos) => {
                // TODO(#1640): 2D labels are not visible in 3D for now.
                if nav_mode == SpatialNavigationMode::ThreeD {
                    continue;
                }
                let pos_in_ui = ui_from_canvas.transform_pos(pos);
                (f32::INFINITY, pos_in_ui + egui::vec2(0.0, 3.0))
            }
            UiLabelTarget::Position3D(pos) => {
                // TODO(#1640): 3D labels are not visible in 2D for now.
                if nav_mode == SpatialNavigationMode::TwoD {
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
        let galley = parent_ui.fonts(|fonts| {
            fonts.layout_job({
                egui::text::LayoutJob {
                    sections: vec![egui::text::LayoutSection {
                        leading_space: 0.0,
                        byte_range: 0..label.text.len(),
                        format: egui::TextFormat::simple(font_id, label.color),
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
            .index_highlight(label.labeled_instance.instance_key);
        let fill_color = match highlight.hover {
            HoverHighlight::None => match highlight.selection {
                SelectionHighlight::None => parent_ui.style().visuals.widgets.inactive.bg_fill,
                SelectionHighlight::SiblingSelection => {
                    parent_ui.style().visuals.widgets.active.bg_fill
                }
                SelectionHighlight::Selection => parent_ui.style().visuals.widgets.active.bg_fill,
            },
            HoverHighlight::Hovered => parent_ui.style().visuals.widgets.hovered.bg_fill,
        };

        label_shapes.push(egui::Shape::rect_filled(bg_rect, 3.0, fill_color));
        label_shapes.push(egui::Shape::galley(text_rect.center_top(), galley));

        ui_rects.push(PickableUiRect {
            rect: ui_from_canvas.inverse().transform_rect(bg_rect),
            instance_hash: label.labeled_instance,
        });
    }

    (label_shapes, ui_rects)
}

pub fn outline_config(gui_ctx: &egui::Context) -> OutlineConfig {
    // Take the exact same colors we have in the ui!
    let selection_outline_color =
        re_renderer::Rgba::from(gui_ctx.style().visuals.selection.bg_fill);
    let hover_outline_color =
        re_renderer::Rgba::from(gui_ctx.style().visuals.widgets.hovered.bg_fill);

    OutlineConfig {
        outline_radius_pixel: (gui_ctx.pixels_per_point() * 1.5).at_least(0.5),
        color_layer_a: hover_outline_color,
        color_layer_b: selection_outline_color,
    }
}

pub fn screenshot_context_menu(
    _ctx: &ViewerContext<'_>,
    response: egui::Response,
) -> (egui::Response, Option<ScreenshotMode>) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        if _ctx.app_options.experimental_space_view_screenshots {
            let mut take_screenshot = None;
            let response = response.context_menu(|ui| {
                if ui.button("Screenshot (save to disk)").clicked() {
                    take_screenshot = Some(ScreenshotMode::SaveAndCopyToClipboard);
                    ui.close_menu();
                } else if ui.button("Screenshot (clipboard only)").clicked() {
                    take_screenshot = Some(ScreenshotMode::CopyToClipboard);
                    ui.close_menu();
                }
            });
            (response, take_screenshot)
        } else {
            (response, None)
        }
    }
    #[cfg(target_arch = "wasm32")]
    {
        (response, None)
    }
}

#[allow(clippy::too_many_arguments)]
pub fn picking(
    ctx: &mut ViewerContext<'_>,
    mut response: egui::Response,
    space_from_ui: egui::emath::RectTransform,
    ui_clip_rect: egui::Rect,
    parent_ui: &mut egui::Ui,
    eye: Eye,
    view_builder: &mut re_renderer::view_builder::ViewBuilder,
    space_view_id: SpaceViewId,
    state: &mut SpatialSpaceViewState,
    scene: &SceneSpatial,
    ui_rects: &[PickableUiRect],
    space: &EntityPath,
) -> egui::Response {
    re_tracing::profile_function!();

    let Some(pointer_pos_ui) = response.hover_pos() else {
        state.previous_picking_result = None;
        return response;
    };

    let picking_context = super::scene::PickingContext::new(
        pointer_pos_ui,
        space_from_ui,
        ui_clip_rect,
        parent_ui.ctx().pixels_per_point(),
        &eye,
    );

    let picking_rect_size =
        super::scene::PickingContext::UI_INTERACTION_RADIUS * parent_ui.ctx().pixels_per_point();
    // Make the picking rect bigger than necessary so we can use it to counter-act delays.
    // (by the time the picking rectangle is read back, the cursor may have moved on).
    let picking_rect_size = (picking_rect_size * 2.0)
        .ceil()
        .at_least(8.0)
        .at_most(128.0) as u32;

    let _ = view_builder.schedule_picking_rect(
        ctx.render_ctx,
        re_renderer::RectInt::from_middle_and_extent(
            picking_context.pointer_in_pixel.as_ivec2(),
            glam::uvec2(picking_rect_size, picking_rect_size),
        ),
        space_view_id.gpu_readback_id(),
        (),
        ctx.app_options.show_picking_debug_overlay,
    );

    let picking_result = picking_context.pick(
        ctx.render_ctx,
        space_view_id.gpu_readback_id(),
        &state.previous_picking_result,
        &scene.parts.images.images,
        ui_rects,
    );
    state.previous_picking_result = Some(picking_result.clone());

    let mut hovered_items = Vec::new();

    // Depth at pointer used for projecting rays from a hovered 2D view to corresponding 3D view(s).
    // TODO(#1818): Depth at pointer only works for depth images so far.
    let mut depth_at_pointer = None;
    for hit in &picking_result.hits {
        let Some(mut instance_path) = hit.instance_path_hash.resolve(&ctx.store_db.entity_db)
            else { continue; };

        if response.double_clicked() {
            // Select entire entity on double-click:
            instance_path.instance_key = re_log_types::InstanceKey::SPLAT;
        }

        if scene
            .context
            .non_interactive_entities
            .0
            .contains(&instance_path.entity_path.hash())
        {
            continue;
        }

        // Special hover ui for images.
        let is_depth_cloud = scene
            .parts
            .images
            .depth_cloud_entities
            .contains(&instance_path.entity_path.hash());
        let picked_image_with_coords = if hit.hit_type == PickingHitType::TexturedRect
            || is_depth_cloud
        {
            let store = &ctx.store_db.entity_db.data_store;
            store
                .query_latest_component::<Tensor>(&instance_path.entity_path, &ctx.current_query())
                .and_then(|tensor| {
                    // If we're here because of back-projection, but this wasn't actually a depth image, drop out.
                    // (the back-projection property may be true despite this not being a depth image!)
                    if hit.hit_type != PickingHitType::TexturedRect
                        && is_depth_cloud
                        && tensor.meaning != TensorDataMeaning::Depth
                    {
                        None
                    } else {
                        tensor.image_height_width_channels().map(|[_, w, _]| {
                            let coordinates = hit
                                .instance_path_hash
                                .instance_key
                                .to_2d_image_coordinate(w);
                            (tensor, coordinates)
                        })
                    }
                })
        } else {
            None
        };
        if picked_image_with_coords.is_some() {
            // We don't support selecting pixels yet.
            instance_path.instance_key = re_log_types::InstanceKey::SPLAT;
        }

        hovered_items.push(Item::InstancePath(
            Some(space_view_id),
            instance_path.clone(),
        ));

        response = if let Some((tensor, coords)) = picked_image_with_coords {
            if let Some(meter) = tensor.meter {
                if let Some(raw_value) = tensor.get(&[
                    picking_context.pointer_in_space2d.y.round() as _,
                    picking_context.pointer_in_space2d.x.round() as _,
                ]) {
                    let raw_value = raw_value.as_f64();
                    let depth_in_meters = raw_value / meter as f64;
                    depth_at_pointer = Some(depth_in_meters as f32);
                }
            }

            response
                .on_hover_cursor(egui::CursorIcon::Crosshair)
                .on_hover_ui_at_pointer(|ui| {
                    ui.set_max_width(320.0);

                    ui.vertical(|ui| {
                        ui.label(instance_path.to_string());
                        instance_path.data_ui(
                            ctx,
                            ui,
                            UiVerbosity::Small,
                            &ctx.current_query(),
                        );

                        if let Some([h, w, ..]) = tensor.image_height_width_channels() {
                            ui.separator();
                            ui.horizontal(|ui| {
                                let (w, h) = (w as f32, h as f32);
                                if *state.nav_mode.get() == SpatialNavigationMode::TwoD {
                                    let rect = egui::Rect::from_min_size(
                                        egui::Pos2::ZERO,
                                        egui::vec2(w, h),
                                    );
                                    show_zoomed_image_region_area_outline(
                                        ui.ctx(),
                                        ui_clip_rect,
                                        &tensor,
                                        [coords[0] as _, coords[1] as _],
                                        space_from_ui.inverse().transform_rect(rect),
                                    );
                                }

                                let tensor_name = instance_path.to_string();

                                let decoded_tensor = ctx.cache.entry(|c: &mut TensorDecodeCache| c.entry(tensor));
                                match decoded_tensor {
                                    Ok(decoded_tensor) => {
                                        let annotations = scene.context.annotations.0.find(&instance_path.entity_path);
                                        let tensor_stats = ctx.cache.entry(|c: &mut TensorStatsCache| c.entry(&decoded_tensor));
                                        show_zoomed_image_region(
                                            ctx.render_ctx,
                                            ui,
                                            &decoded_tensor,
                                            &tensor_stats,
                                            &annotations,
                                            decoded_tensor.meter,
                                            &tensor_name,
                                            [coords[0] as _, coords[1] as _],
                                        );
                                    }
                                    Err(err) => re_log::warn_once!(
                                            "Encountered problem decoding tensor at path {tensor_name}: {err}"
                                        ),
                                }
                            });
                        }
                    });
                })
        } else {
            // Hover ui for everything else
            response.on_hover_ui_at_pointer(|ui| {
                item_ui::instance_path_button(ctx, ui, Some(space_view_id), &instance_path);
                instance_path.data_ui(ctx, ui, UiVerbosity::Reduced, &ctx.current_query());
            })
        };
    }

    item_ui::select_hovered_on_click(&response, ctx.selection_state_mut(), &hovered_items);
    ctx.set_hovered(hovered_items.into_iter());

    let hovered_space = match state.nav_mode.get() {
        SpatialNavigationMode::TwoD => HoveredSpace::TwoD {
            space_2d: space.clone(),
            pos: picking_context
                .pointer_in_space2d
                .extend(depth_at_pointer.unwrap_or(f32::INFINITY)),
        },
        SpatialNavigationMode::ThreeD => {
            let hovered_point = picking_result.space_position();
            HoveredSpace::ThreeD {
                space_3d: space.clone(),
                pos: hovered_point,
                tracked_space_camera: state.state_3d.tracked_camera.clone(),
                point_in_space_cameras: scene
                    .parts
                    .cameras
                    .space_cameras
                    .iter()
                    .map(|cam| {
                        (
                            cam.ent_path.clone(),
                            hovered_point.and_then(|pos| cam.project_onto_2d(pos)),
                        )
                    })
                    .collect(),
            }
        }
    };
    ctx.selection_state_mut().set_hovered_space(hovered_space);

    response
}
