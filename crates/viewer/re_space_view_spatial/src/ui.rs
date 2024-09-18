#![allow(clippy::manual_map)] // Annoying

use egui::{epaint::util::OrderedFloat, text::TextWrapping, NumExt, WidgetText};

use re_data_ui::{
    item_ui, show_zoomed_image_region, show_zoomed_image_region_area_outline, DataUi,
};
use re_format::format_f32;
use re_log_types::Instance;
use re_math::BoundingBox;
use re_renderer::OutlineConfig;
use re_space_view::ScreenshotMode;
use re_types::{
    archetypes::Pinhole,
    blueprint::components::VisualBounds2D,
    components::{DepthMeter, ViewCoordinates},
    image::ImageKind,
};
use re_ui::{
    list_item::{list_item_scope, PropertyContent},
    ContextExt as _, UiExt as _,
};
use re_viewer_context::{
    HoverHighlight, ImageInfo, ImageStatsCache, Item, ItemSpaceContext, SelectionHighlight,
    SpaceViewHighlights, SpaceViewState, SpaceViewSystemExecutionError, UiLayout, ViewQuery,
    ViewerContext, VisualizerCollection,
};

use crate::scene_bounding_boxes::SceneBoundingBoxes;
use crate::{
    contexts::AnnotationSceneContext,
    picking::{PickableUiRect, PickingContext, PickingHitType, PickingResult},
    view_kind::SpatialSpaceViewKind,
    visualizers::{
        iter_spatial_visualizer_data, CamerasVisualizer, DepthImageVisualizer, UiLabel,
        UiLabelTarget,
    },
    PickableTexturedRect,
};
use crate::{eye::EyeMode, pickable_textured_rect::PickableRectSourceData};

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
        self.num_non_segmentation_images_last_frame = iter_spatial_visualizer_data(view_systems)
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
            .index_highlight(label.labeled_instance.instance);
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
            rect: ui_from_scene.inverse().transform_rect(bg_rect),
            instance_hash: label.labeled_instance,
        });
    }

    (label_shapes, ui_rects)
}

pub fn outline_config(gui_ctx: &egui::Context) -> OutlineConfig {
    // Use the exact same colors we have in the ui!
    let hover_outline = gui_ctx.hover_stroke();
    let selection_outline = gui_ctx.selection_stroke();

    // See also: SIZE_BOOST_IN_POINTS_FOR_LINE_OUTLINES

    let outline_radius_ui_pts = 0.5 * f32::max(hover_outline.width, selection_outline.width);
    let outline_radius_pixel = (gui_ctx.pixels_per_point() * outline_radius_ui_pts).at_least(0.5);

    OutlineConfig {
        outline_radius_pixel,
        color_layer_a: re_renderer::Rgba::from(hover_outline.color),
        color_layer_b: re_renderer::Rgba::from(selection_outline.color),
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

#[allow(clippy::too_many_arguments)]
pub fn picking(
    ctx: &ViewerContext<'_>,
    picking_context: &PickingContext,
    ui: &egui::Ui,
    mut response: egui::Response,
    view_builder: &mut re_renderer::view_builder::ViewBuilder,
    state: &mut SpatialSpaceViewState,
    system_output: &re_viewer_context::SystemExecutionOutput,
    ui_rects: &[PickableUiRect],
    query: &ViewQuery<'_>,
    spatial_kind: SpatialSpaceViewKind,
) -> Result<egui::Response, SpaceViewSystemExecutionError> {
    re_tracing::profile_function!();

    if ui.ctx().dragged_id().is_some() {
        state.previous_picking_result = None;
        return Ok(response);
    }

    let Some(render_ctx) = ctx.render_ctx else {
        return Err(SpaceViewSystemExecutionError::NoRenderContextError);
    };

    let picking_rect_size = PickingContext::UI_INTERACTION_RADIUS * ui.ctx().pixels_per_point();
    // Make the picking rect bigger than necessary so we can use it to counter-act delays.
    // (by the time the picking rectangle is read back, the cursor may have moved on).
    let picking_rect_size = (picking_rect_size * 2.0)
        .ceil()
        .at_least(8.0)
        .at_most(128.0) as u32;

    let _ = view_builder.schedule_picking_rect(
        render_ctx,
        re_renderer::RectInt::from_middle_and_extent(
            picking_context.pointer_in_pixel.as_ivec2(),
            glam::uvec2(picking_rect_size, picking_rect_size),
        ),
        query.space_view_id.gpu_readback_id(),
        (),
        ctx.app_options.show_picking_debug_overlay,
    );

    let annotations = system_output
        .context_systems
        .get::<AnnotationSceneContext>()?;

    let picking_result = picking_context.pick(
        render_ctx,
        query.space_view_id.gpu_readback_id(),
        &state.previous_picking_result,
        iter_pickable_rects(&system_output.view_systems),
        ui_rects,
    );
    state.previous_picking_result = Some(picking_result.clone());

    let mut hovered_items = Vec::new();

    // Depth at pointer used for projecting rays from a hovered 2D view to corresponding 3D view(s).
    // TODO(#1818): Depth at pointer only works for depth images so far.
    let mut depth_at_pointer = None;
    for hit in &picking_result.hits {
        let Some(mut instance_path) = hit.instance_path_hash.resolve(ctx.recording()) else {
            // Entity no longer exists in db.
            continue;
        };

        if response.double_clicked() {
            // Select entire entity on double-click:
            instance_path.instance = Instance::ALL;
        }

        let query_result = ctx.lookup_query_result(query.space_view_id);
        let Some(data_result) = query_result
            .tree
            .lookup_result_by_path(&instance_path.entity_path)
        else {
            // No data result for this entity means it's no longer on screen.
            continue;
        };

        if !data_result.is_interactive(ctx) {
            continue;
        }

        response = if let Some(picked_image) = get_image_picking_info(system_output, hit) {
            // We don't support selecting pixels yet.
            instance_path.instance = Instance::ALL;

            if picked_image.kind() == ImageKind::Depth {
                if let Some(meter) = picked_image.depth_meter {
                    let [x, y] = picked_image.coordinates;
                    if let Some(raw_value) = picked_image.image.get_xyc(x, y, 0) {
                        let raw_value = raw_value.as_f64();
                        let depth_in_meters = raw_value / *meter.0 as f64;
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
                            ctx,
                            ui,
                            &instance_path,
                            spatial_kind,
                            picking_context.ui_pan_and_zoom_from_ui,
                            annotations,
                            picked_image,
                        );
                    });
                })
        } else {
            // Hover ui for everything else
            response.on_hover_ui_at_pointer(|ui| {
                list_item_scope(ui, "spatial_hover", |ui| {
                    hit_ui(ui, hit);
                    item_ui::instance_path_button(
                        ctx,
                        &query.latest_at_query(),
                        ctx.recording(),
                        ui,
                        Some(query.space_view_id),
                        &instance_path,
                    );
                    instance_path.data_ui_recording(ctx, ui, UiLayout::Tooltip);
                });
            })
        };

        hovered_items.push(Item::DataResult(query.space_view_id, instance_path.clone()));
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
            SpatialSpaceViewKind::TwoD => ItemSpaceContext::TwoD {
                space_2d: query.space_origin.clone(),
                pos: picking_context
                    .pointer_in_ui_after_pan_and_zoom
                    .extend(depth_at_pointer.unwrap_or(f32::INFINITY)),
            },
            SpatialSpaceViewKind::ThreeD => {
                let hovered_point = picking_result.space_position();
                let cameras_visualizer_output =
                    system_output.view_systems.get::<CamerasVisualizer>()?;

                ItemSpaceContext::ThreeD {
                    space_3d: query.space_origin.clone(),
                    pos: hovered_point,
                    tracked_entity: state.state_3d.tracked_entity.clone(),
                    point_in_space_cameras: cameras_visualizer_output
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

    ctx.select_hovered_on_click(&response, hovered_items.into_iter());

    Ok(response)
}

fn iter_pickable_rects(
    visualizers: &VisualizerCollection,
) -> impl Iterator<Item = &PickableTexturedRect> {
    iter_spatial_visualizer_data(visualizers).flat_map(|data| data.pickable_rects.iter())
}

/// If available, finds image info for a picking hit.
fn get_image_picking_info(
    system_output: &re_viewer_context::SystemExecutionOutput,
    hit: &crate::picking::PickingRayHit,
) -> Option<PickedImageInfo> {
    let depth_visualizer_output = system_output
        .view_systems
        .get::<DepthImageVisualizer>()
        .ok();

    if hit.hit_type == PickingHitType::TexturedRect {
        iter_pickable_rects(&system_output.view_systems)
            .find(|i| i.ent_path.hash() == hit.instance_path_hash.entity_path_hash)
            .and_then(|picked_rect| {
                let coordinates = hit
                    .instance_path_hash
                    .instance
                    .to_2d_image_coordinate(picked_rect.pixel_width());

                if let PickableRectSourceData::Image { image, depth_meter } =
                    &picked_rect.source_data
                {
                    Some(PickedImageInfo {
                        image: image.clone(),
                        coordinates,
                        depth_meter: *depth_meter,
                    })
                } else {
                    None
                }
            })
    } else if let Some((depth_image, depth_meter)) =
        depth_visualizer_output.and_then(|depth_images| {
            depth_images
                .depth_cloud_entities
                .get(&hit.instance_path_hash.entity_path_hash)
        })
    {
        let coordinates = hit
            .instance_path_hash
            .instance
            .to_2d_image_coordinate(depth_image.width() as _);
        Some(PickedImageInfo {
            image: depth_image.clone(),
            coordinates,
            depth_meter: Some(*depth_meter),
        })
    } else {
        None
    }
}

struct PickedImageInfo {
    image: ImageInfo,
    coordinates: [u32; 2],
    depth_meter: Option<DepthMeter>,
}

impl PickedImageInfo {
    pub fn kind(&self) -> ImageKind {
        self.image.kind
    }
}

#[allow(clippy::too_many_arguments)]
fn image_hover_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    instance_path: &re_entity_db::InstancePath,
    spatial_kind: SpatialSpaceViewKind,
    ui_pan_and_zoom_from_ui: egui::emath::RectTransform,
    annotations: &AnnotationSceneContext,
    picked_image_info: PickedImageInfo,
) {
    let PickedImageInfo {
        image,
        coordinates,
        depth_meter,
    } = picked_image_info;

    let depth_meter = depth_meter.map(|d| *d.0);

    ui.label(instance_path.to_string());

    let (w, h) = (image.format.width as u64, image.format.height as u64);

    ui.add_space(8.0);

    ui.horizontal(|ui| {
        let (w, h) = (w as f32, h as f32);

        if spatial_kind == SpatialSpaceViewKind::TwoD {
            let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(w, h));

            show_zoomed_image_region_area_outline(
                ui.ctx(),
                *ui_pan_and_zoom_from_ui.from(),
                egui::vec2(w, h),
                [coordinates[0] as _, coordinates[1] as _],
                ui_pan_and_zoom_from_ui.inverse().transform_rect(rect),
            );
        }

        let tensor_name = instance_path.to_string();

        let annotations = annotations.0.find(&instance_path.entity_path);

        let tensor_stats = ctx.cache.entry(|c: &mut ImageStatsCache| c.entry(&image));
        if let Some(render_ctx) = ctx.render_ctx {
            show_zoomed_image_region(
                render_ctx,
                ui,
                &image,
                &tensor_stats,
                &annotations,
                depth_meter,
                &tensor_name,
                [coordinates[0] as _, coordinates[1] as _],
            );
        }
    });
}

fn hit_ui(ui: &mut egui::Ui, hit: &crate::picking::PickingRayHit) {
    if hit.hit_type == PickingHitType::GpuPickingResult {
        let glam::Vec3 { x, y, z } = hit.space_position;
        ui.list_item_flat_noninteractive(PropertyContent::new("Hover position").value_fn(
            |ui, _| {
                ui.add(egui::Label::new(format!("[{x:.5}, {y:.5}, {z:.5}]")).extend());
            },
        ));
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
