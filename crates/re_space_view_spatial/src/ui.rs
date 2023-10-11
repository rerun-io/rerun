use egui::{epaint::util::OrderedFloat, text::TextWrapping, NumExt, WidgetText};
use macaw::BoundingBox;

use re_data_ui::{image_meaning_for_entity, item_ui, DataUi};
use re_data_ui::{show_zoomed_image_region, show_zoomed_image_region_area_outline};
use re_entity_db::EntityPath;
use re_format::format_f32;
use re_renderer::OutlineConfig;
use re_space_view::ScreenshotMode;
use re_types::components::{DepthMeter, InstanceKey, TensorData, ViewCoordinates};
use re_types::tensor_data::TensorDataMeaning;
use re_types::view_coordinates::SignedAxis3;
use re_viewer_context::{
    HoverHighlight, Item, SelectedSpaceContext, SelectionHighlight, SpaceViewHighlights,
    SpaceViewState, SpaceViewSystemExecutionError, TensorDecodeCache, TensorStatsCache,
    UiVerbosity, ViewContextCollection, ViewQuery, ViewerContext, VisualizerCollection,
};

use super::{eye::Eye, ui_2d::View2DState, ui_3d::View3DState};
use crate::heuristics::auto_size_world_heuristic;
use crate::scene_bounding_boxes::SceneBoundingBoxes;
use crate::{
    contexts::{AnnotationSceneContext, NonInteractiveEntities},
    picking::{PickableUiRect, PickingContext, PickingHitType, PickingResult},
    view_kind::SpatialSpaceViewKind,
    visualizers::{CamerasVisualizer, ImageVisualizer, UiLabel, UiLabelTarget},
};

/// Default auto point radius in UI points.
const AUTO_POINT_RADIUS: f32 = 1.5;

/// Default auto line radius in UI points.
const AUTO_LINE_RADIUS: f32 = 1.5;

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

/// TODO(andreas): Should turn this "inside out" - [`SpatialSpaceViewState`] should be used by [`View2DState`]/[`View3DState`], not the other way round.
#[derive(Clone, Default)]
pub struct SpatialSpaceViewState {
    pub bounding_boxes: SceneBoundingBoxes,

    /// Estimated number of primitives last frame. Used to inform some heuristics.
    pub scene_num_primitives: usize,

    /// Last frame's picking result.
    pub previous_picking_result: Option<PickingResult>,

    pub(super) state_2d: View2DState,
    pub(super) state_3d: View3DState,

    /// Size of automatically sized objects. None if it wasn't configured.
    auto_size_config: re_renderer::AutoSizeConfig,
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

    pub fn selection_ui(
        &mut self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        space_origin: &EntityPath,
        spatial_kind: SpatialSpaceViewKind,
    ) {
        let re_ui = ctx.re_ui;

        let scene_view_coordinates = ctx
            .entity_db
            .store()
            .query_latest_component::<ViewCoordinates>(space_origin, &ctx.current_query())
            .map(|c| c.value);

        ctx.re_ui.selection_grid(ui, "spatial_settings_ui")
            .show(ui, |ui| {
            let auto_size_world = auto_size_world_heuristic(&self.bounding_boxes.accumulated, self.scene_num_primitives);

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
                    .on_hover_text("Point radius used whenever not explicitly specified");
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
                            .on_hover_text("Line radius used whenever not explicitly specified");
                    });
                });
            });
            ui.end_row();

            ctx.re_ui.grid_left_hand_label(ui, "Camera")
                .on_hover_text("The virtual camera which controls what is shown on screen");
            ui.vertical(|ui| {
                if spatial_kind == SpatialSpaceViewKind::ThreeD {
                    if ui.button("Reset").on_hover_text(
                        "Resets camera position & orientation.\nYou can also double-click the 3D view.")
                        .clicked()
                    {
                        self.bounding_boxes.accumulated = self.bounding_boxes.current;
                        self.state_3d.reset_camera(&self.bounding_boxes.accumulated, scene_view_coordinates);
                    }
                    let mut spin = self.state_3d.spin();
                    if re_ui.checkbox(ui, &mut spin, "Spin")
                        .on_hover_text("Spin camera around the orbit center").changed() {
                        self.state_3d.set_spin(spin);
                    }
                }
            });
            ui.end_row();

            if spatial_kind == SpatialSpaceViewKind::ThreeD {
                ctx.re_ui.grid_left_hand_label(ui, "Coordinates")
                    .on_hover_text("The world coordinate system used for this view");
                ui.vertical(|ui|{
                    // TODO(#3816): We should also display the current eye's up axis.
                    let up_description = if let Some(scene_up) = scene_view_coordinates.and_then(|vc| vc.up()) {
                        format!("Scene up is {scene_up}")
                    } else {
                        let scene_up = SignedAxis3::POSITIVE_Z; // defauls to RUF
                        format!("Scene up is unspecified (defaulting to {scene_up})")
                    };
                    ui.label(up_description).on_hover_ui(|ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 0.0;
                            ui.label("Set with ");
                            ui.code("rerun.ViewCoordinates");
                            ui.label(".");
                        });
                    });
                    re_ui.checkbox(ui, &mut self.state_3d.show_axes, "Show origin axes").on_hover_text("Show X-Y-Z axes");
                    re_ui.checkbox(ui, &mut self.state_3d.show_bbox, "Show bounding box").on_hover_text("Show the current scene bounding box");
                    re_ui.checkbox(ui, &mut self.state_3d.show_accumulated_bbox, "Show accumulated bounding box").on_hover_text("Show bounding box accumulated over all rendered frames");
                });
                ui.end_row();
            }

            ctx.re_ui.grid_left_hand_label(ui, "Bounding box")
                .on_hover_text("The bounding box encompassing all Entities in the view right now");
            ui.vertical(|ui| {
                ui.style_mut().wrap = Some(false);
                let BoundingBox { min, max } = self.bounding_boxes.current;
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
                if spatial_kind == SpatialSpaceViewKind::ThreeD {
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
                .on_hover_text("Determine automatically");
            ui.selectable_value(&mut mode, AutoSizeUnit::UiPoints, AutoSizeUnit::UiPoints)
                .on_hover_text("Manual in UI points");
            ui.selectable_value(&mut mode, AutoSizeUnit::World, AutoSizeUnit::World)
                .on_hover_text("Manual in scene units");
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

pub fn create_labels(
    mut labels: Vec<UiLabel>,
    ui_from_canvas: egui::emath::RectTransform,
    eye3d: &Eye,
    parent_ui: &egui::Ui,
    highlights: &SpaceViewHighlights,
    spatial_kind: SpatialSpaceViewKind,
) -> (Vec<egui::Shape>, Vec<PickableUiRect>) {
    re_tracing::profile_function!();

    let ui_from_world_3d = eye3d.ui_from_world(*ui_from_canvas.to());

    // Closest last (painters algorithm)
    labels.sort_by_key(|label| {
        if let UiLabelTarget::Position3D(pos) = label.target {
            OrderedFloat::from(-ui_from_world_3d.transform_point3(pos).z)
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
                let rect_in_ui = ui_from_canvas.transform_rect(rect);
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
                let pos_in_ui = ui_from_canvas.transform_pos(pos);
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
        label_shapes.push(egui::Shape::galley(
            text_rect.center_top(),
            galley,
            label.color,
        ));

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
    _response: &egui::Response,
) -> Option<ScreenshotMode> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        if _ctx.app_options.experimental_space_view_screenshots {
            let mut take_screenshot = None;
            _response.context_menu(|ui| {
                ui.style_mut().wrap = Some(false);
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

#[allow(clippy::too_many_arguments)] // TODO(andreas): Make this method sane.
pub fn picking(
    ctx: &ViewerContext<'_>,
    mut response: egui::Response,
    space_from_ui: egui::emath::RectTransform,
    ui_clip_rect: egui::Rect,
    parent_ui: &egui::Ui,
    eye: Eye,
    view_builder: &mut re_renderer::view_builder::ViewBuilder,
    state: &mut SpatialSpaceViewState,
    view_ctx: &ViewContextCollection,
    visualizers: &VisualizerCollection,
    ui_rects: &[PickableUiRect],
    query: &ViewQuery<'_>,
    spatial_kind: SpatialSpaceViewKind,
) -> Result<egui::Response, SpaceViewSystemExecutionError> {
    re_tracing::profile_function!();

    let Some(pointer_pos_ui) = response.hover_pos() else {
        state.previous_picking_result = None;
        return Ok(response);
    };

    let picking_context = PickingContext::new(
        pointer_pos_ui,
        space_from_ui,
        ui_clip_rect,
        parent_ui.ctx().pixels_per_point(),
        &eye,
    );

    let picking_rect_size =
        PickingContext::UI_INTERACTION_RADIUS * parent_ui.ctx().pixels_per_point();
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
        query.space_view_id.gpu_readback_id(),
        (),
        ctx.app_options.show_picking_debug_overlay,
    );

    let non_interactive = view_ctx.get::<NonInteractiveEntities>()?;
    let annotations = view_ctx.get::<AnnotationSceneContext>()?;
    let images = visualizers.get::<ImageVisualizer>()?;

    let picking_result = picking_context.pick(
        ctx.render_ctx,
        query.space_view_id.gpu_readback_id(),
        &state.previous_picking_result,
        &images.images,
        ui_rects,
    );
    state.previous_picking_result = Some(picking_result.clone());

    let mut hovered_items = Vec::new();

    // Depth at pointer used for projecting rays from a hovered 2D view to corresponding 3D view(s).
    // TODO(#1818): Depth at pointer only works for depth images so far.
    let mut depth_at_pointer = None;
    for hit in &picking_result.hits {
        let Some(mut instance_path) = hit.instance_path_hash.resolve(ctx.entity_db) else {
            continue;
        };

        if response.double_clicked() {
            // Select entire entity on double-click:
            instance_path.instance_key = InstanceKey::SPLAT;
        }

        if non_interactive
            .0
            .contains(&instance_path.entity_path.hash())
        {
            continue;
        }

        let store = ctx.entity_db.store();

        // Special hover ui for images.
        let is_depth_cloud = images
            .depth_cloud_entities
            .contains(&instance_path.entity_path.hash());
        let picked_image_with_coords =
            if hit.hit_type == PickingHitType::TexturedRect || is_depth_cloud {
                let meaning = image_meaning_for_entity(
                    &instance_path.entity_path,
                    &query.latest_at_query(),
                    store,
                );

                store
                    .query_latest_component::<TensorData>(
                        &instance_path.entity_path,
                        &ctx.current_query(),
                    )
                    .and_then(|tensor| {
                        // If we're here because of back-projection, but this wasn't actually a depth image, drop out.
                        // (the back-projection property may be true despite this not being a depth image!)
                        if hit.hit_type != PickingHitType::TexturedRect
                            && is_depth_cloud
                            && meaning != TensorDataMeaning::Depth
                        {
                            None
                        } else {
                            let tensor_path_hash = hit.instance_path_hash.versioned(tensor.row_id);
                            tensor.image_height_width_channels().map(|[_, w, _]| {
                                let coordinates = hit
                                    .instance_path_hash
                                    .instance_key
                                    .to_2d_image_coordinate(w);
                                (tensor_path_hash, tensor, meaning, coordinates)
                            })
                        }
                    })
            } else {
                None
            };
        if picked_image_with_coords.is_some() {
            // We don't support selecting pixels yet.
            instance_path.instance_key = InstanceKey::SPLAT;
        }

        hovered_items.push(Item::InstancePath(
            Some(query.space_view_id),
            instance_path.clone(),
        ));

        response = if let Some((tensor_path_hash, tensor, meaning, coords)) =
            picked_image_with_coords
        {
            let meter = store
                .query_latest_component::<DepthMeter>(
                    &instance_path.entity_path,
                    &ctx.current_query(),
                )
                .map(|meter| meter.value.0);

            // TODO(jleibs): Querying this here feels weird. Would be nice to do this whole
            // thing as an up-front archetype query somewhere.
            if meaning == TensorDataMeaning::Depth {
                if let Some(meter) = meter {
                    let [x, y] = coords;
                    if let Some(raw_value) = tensor.get(&[y as _, x as _]) {
                        let raw_value = raw_value.as_f64();
                        let depth_in_meters = raw_value / meter as f64;
                        depth_at_pointer = Some(depth_in_meters as f32);
                    }
                }
            }

            response
                .on_hover_cursor(egui::CursorIcon::Crosshair)
                .on_hover_ui_at_pointer(|ui| {
                    ui.set_max_width(320.0);
                    ui.vertical(|ui| {
                        image_hover_ui(
                            ui,
                            &instance_path,
                            ctx,
                            tensor.value,
                            spatial_kind,
                            ui_clip_rect,
                            coords,
                            space_from_ui,
                            tensor_path_hash.row_id,
                            annotations,
                            meaning,
                            meter,
                        );
                    });
                })
        } else {
            // Hover ui for everything else
            response.on_hover_ui_at_pointer(|ui| {
                hit_ui(ui, hit);
                item_ui::instance_path_button(
                    ctx,
                    &query.latest_at_query(),
                    store,
                    ui,
                    Some(query.space_view_id),
                    &instance_path,
                );
                instance_path.data_ui(ctx, ui, UiVerbosity::Reduced, &ctx.current_query(), store);
            })
        };
    }

    if hovered_items.is_empty() {
        // If we hover nothing, we are hovering the space-view itself.
        hovered_items.push(Item::SpaceView(query.space_view_id));
    }

    // Associate the hovered space with the first item in the hovered item list.
    // If we were to add several, space views might render unnecessary additional hints.
    // TODO(andreas): Should there be context if no item is hovered at all? There's no usecase for that today it seems.
    let mut hovered_items = hovered_items
        .into_iter()
        .map(|item| (item, None))
        .collect::<Vec<_>>();

    if let Some((_, context)) = hovered_items.first_mut() {
        *context = Some(match spatial_kind {
            SpatialSpaceViewKind::TwoD => SelectedSpaceContext::TwoD {
                space_2d: query.space_origin.clone(),
                pos: picking_context
                    .pointer_in_space2d
                    .extend(depth_at_pointer.unwrap_or(f32::INFINITY)),
            },
            SpatialSpaceViewKind::ThreeD => {
                let hovered_point = picking_result.space_position();
                SelectedSpaceContext::ThreeD {
                    space_3d: query.space_origin.clone(),
                    pos: hovered_point,
                    tracked_entity: state.state_3d.tracked_entity.clone(),
                    point_in_space_cameras: visualizers
                        .get::<CamerasVisualizer>()?
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
        });
    };

    ctx.select_hovered_on_click(&response, re_viewer_context::Selection(hovered_items));

    Ok(response)
}

#[allow(clippy::too_many_arguments)]
fn image_hover_ui(
    ui: &mut egui::Ui,
    instance_path: &re_entity_db::InstancePath,
    ctx: &ViewerContext<'_>,
    tensor: TensorData,
    spatial_kind: SpatialSpaceViewKind,
    ui_clip_rect: egui::Rect,
    coords: [u32; 2],
    space_from_ui: egui::emath::RectTransform,
    tensor_data_row_id: re_log_types::RowId,
    annotations: &AnnotationSceneContext,
    meaning: TensorDataMeaning,
    meter: Option<f32>,
) {
    ui.label(instance_path.to_string());
    if true {
        // Only show the `TensorData` component, to keep the hover UI small; see https://github.com/rerun-io/rerun/issues/3573
        use re_types::Loggable as _;
        let component_path = re_log_types::ComponentPath::new(
            instance_path.entity_path.clone(),
            re_types::components::TensorData::name(),
        );
        component_path.data_ui(
            ctx,
            ui,
            UiVerbosity::Small,
            &ctx.current_query(),
            ctx.entity_db.store(),
        );
    } else {
        // Show it all, like we do for any other thing we hover
        instance_path.data_ui(
            ctx,
            ui,
            UiVerbosity::Small,
            &ctx.current_query(),
            ctx.entity_db.store(),
        );
    }

    if let Some([h, w, ..]) = tensor.image_height_width_channels() {
        ui.separator();
        ui.horizontal(|ui| {
            let (w, h) = (w as f32, h as f32);
            if spatial_kind == SpatialSpaceViewKind::TwoD {
                let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(w, h));
                show_zoomed_image_region_area_outline(
                    ui.ctx(),
                    ui_clip_rect,
                    &tensor.0,
                    [coords[0] as _, coords[1] as _],
                    space_from_ui.inverse().transform_rect(rect),
                );
            }

            let tensor_name = instance_path.to_string();

            let decoded_tensor = ctx
                .cache
                .entry(|c: &mut TensorDecodeCache| c.entry(tensor_data_row_id, tensor.0));
            match decoded_tensor {
                Ok(decoded_tensor) => {
                    let annotations = annotations.0.find(&instance_path.entity_path);
                    let tensor_stats = ctx.cache.entry(|c: &mut TensorStatsCache| {
                        c.entry(tensor_data_row_id, &decoded_tensor)
                    });
                    show_zoomed_image_region(
                        ctx.render_ctx,
                        ui,
                        tensor_data_row_id,
                        &decoded_tensor,
                        &tensor_stats,
                        &annotations,
                        meaning,
                        meter,
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
}

fn hit_ui(ui: &mut egui::Ui, hit: &crate::picking::PickingRayHit) {
    if hit.hit_type == PickingHitType::GpuPickingResult {
        let glam::Vec3 { x, y, z } = hit.space_position;
        ui.label(format!("Hover position: [{x:.5}, {y:.5}, {z:.5}]"));
    }
}
