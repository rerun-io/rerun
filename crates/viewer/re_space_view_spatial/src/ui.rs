#![allow(clippy::manual_map)] // Annoying

use std::sync::Arc;

use egui::{epaint::util::OrderedFloat, text::TextWrapping, NumExt, WidgetText};
use itertools::chain;
use re_math::BoundingBox;

use re_data_ui::{
    item_ui, show_zoomed_image_region, show_zoomed_image_region_area_outline,
    show_zoomed_tensor_region, DataUi,
};
use re_format::format_f32;
use re_log_types::Instance;
use re_renderer::OutlineConfig;
use re_space_view::{latest_at_with_blueprint_resolved_data, ScreenshotMode};
use re_types::{
    archetypes::Pinhole,
    blueprint::components::VisualBounds2D,
    components::{
        Blob, ChannelDataType, Colormap, DepthMeter, Resolution2D, TensorData, ViewCoordinates,
    },
    tensor_data::TensorDataMeaning,
    Loggable as _,
};
use re_ui::{
    list_item::{list_item_scope, PropertyContent},
    ContextExt as _, UiExt as _,
};
use re_viewer_context::{
    HoverHighlight, ImageInfo, ImageStatsCache, Item, ItemSpaceContext, SelectionHighlight,
    SpaceViewHighlights, SpaceViewState, SpaceViewSystemExecutionError, TensorStatsCache, UiLayout,
    ViewContext, ViewContextCollection, ViewQuery, ViewerContext, VisualizerCollection,
};
use re_viewport_blueprint::SpaceViewBlueprint;

use crate::eye::EyeMode;
use crate::scene_bounding_boxes::SceneBoundingBoxes;
use crate::{
    contexts::AnnotationSceneContext,
    picking::{PickableUiRect, PickingContext, PickingHitType, PickingResult},
    view_kind::SpatialSpaceViewKind,
    visualizers::{
        CamerasVisualizer, DepthImageVisualizer, ImageEncodedVisualizer, ImageVisualizer,
        SegmentationImageVisualizer, UiLabel, UiLabelTarget,
    },
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
    ) -> Result<(), SpaceViewSystemExecutionError> {
        re_tracing::profile_function!();

        self.bounding_boxes.update(ui, &system_output.view_systems);

        let view_systems = &system_output.view_systems;

        self.num_non_segmentation_images_last_frame +=
            view_systems.get::<ImageEncodedVisualizer>()?.images.len();
        self.num_non_segmentation_images_last_frame +=
            view_systems.get::<ImageVisualizer>()?.images.len();
        self.num_non_segmentation_images_last_frame +=
            view_systems.get::<DepthImageVisualizer>()?.images.len();

        Ok(())
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
            ui.horizontal(|ui| {
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
    visualizers: &Arc<VisualizerCollection>,
    ui_rects: &[PickableUiRect],
    query: &ViewQuery<'_>,
    spatial_kind: SpatialSpaceViewKind,
) -> Result<egui::Response, SpaceViewSystemExecutionError> {
    re_tracing::profile_function!();

    let Some(pointer_pos_ui) = response.hover_pos() else {
        state.previous_picking_result = None;
        return Ok(response);
    };

    let Some(render_ctx) = ctx.render_ctx else {
        return Err(SpaceViewSystemExecutionError::NoRenderContextError);
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
        render_ctx,
        re_renderer::RectInt::from_middle_and_extent(
            picking_context.pointer_in_pixel.as_ivec2(),
            glam::uvec2(picking_rect_size, picking_rect_size),
        ),
        query.space_view_id.gpu_readback_id(),
        (),
        ctx.app_options.show_picking_debug_overlay,
    );

    let annotations = view_ctx.get::<AnnotationSceneContext>()?;

    let depth_images = visualizers.get::<DepthImageVisualizer>()?;
    let images = visualizers.get::<ImageVisualizer>()?;
    let images_encoded = visualizers.get::<ImageEncodedVisualizer>()?;
    let segmentation_images = visualizers.get::<SegmentationImageVisualizer>()?;
    let image_picking_rects = itertools::chain!(
        &depth_images.images,
        &images.images,
        &images_encoded.images,
        &segmentation_images.images,
    );

    let picking_result = picking_context.pick(
        render_ctx,
        query.space_view_id.gpu_readback_id(),
        &state.previous_picking_result,
        image_picking_rects,
        ui_rects,
    );
    state.previous_picking_result = Some(picking_result.clone());

    let mut hovered_items = Vec::new();

    // TODO(andreas): Should defaults path be always created on the fly?
    let defaults_path = SpaceViewBlueprint::defaults_path(query.space_view_id);
    let view_ctx = ViewContext {
        viewer_ctx: ctx,
        view_id: query.space_view_id,
        view_state: state,
        defaults_path: &defaults_path,
        visualizer_collection: visualizers.clone(),
    };

    // Depth at pointer used for projecting rays from a hovered 2D view to corresponding 3D view(s).
    // TODO(#1818): Depth at pointer only works for depth images so far.
    let mut depth_at_pointer = None;
    for hit in &picking_result.hits {
        let Some(mut instance_path) = hit.instance_path_hash.resolve(ctx.recording()) else {
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
            continue; // No data result for this entity, meaning it's no longer on screen.
        };

        if !data_result.is_interactive(ctx) {
            continue;
        }

        // Special hover ui for images.
        let is_depth_cloud = depth_images
            .depth_cloud_entities
            .contains(&instance_path.entity_path.hash());

        let picked_image = if hit.hit_type == PickingHitType::TexturedRect || is_depth_cloud {
            if let Some(picked) = chain!(
                depth_images.images.iter(),
                // images.images.iter(), // TODO(#6386)
                images_encoded.images.iter(),
                // segmentation_images.images.iter(), // TODO(#6386)
            )
            .find(|i| i.ent_path == instance_path.entity_path)
            {
                picked.width().and_then(|width| {
                    let coordinates = hit
                        .instance_path_hash
                        .instance
                        .to_2d_image_coordinate(width);

                    if let Some(image) = picked.image.clone() {
                        Some(PickedImageInfo {
                            row_id: picked.row_id,
                            meaning: picked.meaning,
                            coordinates,
                            colormap: image.colormap.unwrap_or_default(),
                            depth_meter: picked.depth_meter,
                            tensor: None,
                            image: Some(image),
                        })
                    } else if let Some(tensor) = picked.tensor.clone() {
                        Some(PickedImageInfo {
                            row_id: picked.row_id,
                            meaning: picked.meaning,
                            coordinates,
                            colormap: Default::default(),
                            depth_meter: picked.depth_meter,
                            tensor: Some(tensor),
                            image: None,
                        })
                    } else {
                        None
                    }
                })
            } else {
                let meaning = if segmentation_images
                    .images
                    .iter()
                    .any(|i| i.ent_path == instance_path.entity_path)
                {
                    TensorDataMeaning::ClassId
                } else if is_depth_cloud
                    || depth_images
                        .images
                        .iter()
                        .any(|i| i.ent_path == instance_path.entity_path)
                {
                    TensorDataMeaning::Depth
                } else {
                    TensorDataMeaning::Unknown
                };

                if hit.hit_type != PickingHitType::TexturedRect
                    && is_depth_cloud
                    && meaning != TensorDataMeaning::Depth
                {
                    // If we're here because of back-projection, but this wasn't actually a depth image, drop out.
                    // (the back-projection property may be true despite this not being a depth image!)
                    None
                } else {
                    picked_image_from_image_query(&view_ctx, data_result, hit, meaning).or_else(
                        || picked_image_from_tensor_query(&view_ctx, data_result, hit, meaning),
                    )
                }
            }
        } else {
            None
        };
        if picked_image.is_some() {
            // We don't support selecting pixels yet.
            instance_path.instance = Instance::ALL;
        }

        hovered_items.push(Item::DataResult(query.space_view_id, instance_path.clone()));

        response = if let Some(image_info) = picked_image {
            // TODO(jleibs): Querying this here feels weird. Would be nice to do this whole
            // thing as an up-front archetype query somewhere.
            if image_info.meaning == TensorDataMeaning::Depth {
                if let Some(meter) = image_info.depth_meter {
                    let [x, y] = image_info.coordinates;
                    if let Some(image) = &image_info.image {
                        if let Some(raw_value) = image.get_xyc(x, y, 0) {
                            let raw_value = raw_value.as_f64();
                            let depth_in_meters = raw_value / *meter.0 as f64;
                            depth_at_pointer = Some(depth_in_meters as f32);
                        }
                    }

                    if let Some(tensor) = &image_info.tensor {
                        if let Some(raw_value) = tensor.get(&[y as _, x as _]) {
                            let raw_value = raw_value.as_f64();
                            let depth_in_meters = raw_value / *meter.0 as f64;
                            depth_at_pointer = Some(depth_in_meters as f32);
                        }
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
                            ui_clip_rect,
                            space_from_ui,
                            annotations,
                            image_info,
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
                    .pointer_in_space2d
                    .extend(depth_at_pointer.unwrap_or(f32::INFINITY)),
            },
            SpatialSpaceViewKind::ThreeD => {
                let hovered_point = picking_result.space_position();
                ItemSpaceContext::ThreeD {
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

    ctx.select_hovered_on_click(&response, hovered_items.into_iter());

    Ok(response)
}

fn picked_image_from_tensor_query(
    view_ctx: &ViewContext<'_>,
    data_result: &re_viewer_context::DataResult,
    hit: &crate::picking::PickingRayHit,
    meaning: TensorDataMeaning,
) -> Option<PickedImageInfo> {
    let query_shadowed_defaults = false;
    let results = latest_at_with_blueprint_resolved_data(
        view_ctx,
        None,
        &view_ctx.viewer_ctx.current_query(),
        data_result,
        [TensorData::name(), Colormap::name(), DepthMeter::name()],
        query_shadowed_defaults,
    );

    // TODO(andreas): Just calling `results.get_mono::<TensorData>` would be a lot more elegant.
    // However, we're in the rare case where we really want a RowId to be able to identify the tensor for caching purposes.
    let tensor_untyped = results.get(TensorData::name())?;
    let tensor = tensor_untyped.mono::<TensorData>(&results.resolver)?;

    tensor.image_height_width_channels().map(|[_, w, _]| {
        let (_, row_id) = *tensor_untyped.index();
        let coordinates = hit.instance_path_hash.instance.to_2d_image_coordinate(w);

        PickedImageInfo {
            row_id,
            tensor: Some(tensor.0),
            image: None,
            meaning,
            coordinates,
            colormap: results.get_mono_with_fallback::<Colormap>(),
            depth_meter: results.get_mono::<DepthMeter>(),
        }
    })
}

fn picked_image_from_image_query(
    view_ctx: &ViewContext<'_>,
    data_result: &re_viewer_context::DataResult,
    hit: &crate::picking::PickingRayHit,
    meaning: TensorDataMeaning,
) -> Option<PickedImageInfo> {
    let query_shadowed_defaults = false;
    let results = latest_at_with_blueprint_resolved_data(
        view_ctx,
        None,
        &view_ctx.viewer_ctx.current_query(),
        data_result,
        [
            Blob::name(),
            Resolution2D::name(),
            ChannelDataType::name(),
            Colormap::name(),
            DepthMeter::name(),
        ],
        query_shadowed_defaults,
    );

    // TODO(andreas): Just calling `results.get_mono::<Blob>` would be a lot more elegant.
    // However, we're in the rare case where we really want a RowId to be able to identify the tensor for caching purposes.
    let blob_untyped = results.get(Blob::name())?;
    let blob = blob_untyped.mono::<Blob>(&results.resolver)?.0;

    let resolution = results.get_mono::<Resolution2D>()?;
    let data_type = results.get_mono::<ChannelDataType>()?;
    let colormap = results.get_mono_with_fallback::<Colormap>();
    let depth_meter = results.get_mono::<DepthMeter>();

    let (_, blob_row_id) = *blob_untyped.index();
    let coordinates = hit
        .instance_path_hash
        .instance
        .to_2d_image_coordinate(resolution.width() as _);

    let image = ImageInfo {
        blob_row_id,
        blob,
        resolution: resolution.0.into(),
        data_type,
        color_model: None,
        colormap: Some(colormap),
    };

    Some(PickedImageInfo {
        row_id: blob_row_id,
        tensor: None,
        image: Some(image),
        meaning,
        coordinates,
        colormap,
        depth_meter,
    })
}

struct PickedImageInfo {
    row_id: re_chunk_store::RowId,
    meaning: TensorDataMeaning,
    coordinates: [u32; 2],
    colormap: Colormap,
    depth_meter: Option<DepthMeter>,
    tensor: Option<re_types::datatypes::TensorData>,
    image: Option<ImageInfo>,
}

#[allow(clippy::too_many_arguments)]
fn image_hover_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    instance_path: &re_entity_db::InstancePath,
    spatial_kind: SpatialSpaceViewKind,
    ui_clip_rect: egui::Rect,
    space_from_ui: egui::emath::RectTransform,
    annotations: &AnnotationSceneContext,
    picked_image_info: PickedImageInfo,
) {
    let PickedImageInfo {
        row_id,
        meaning,
        coordinates,
        colormap,
        depth_meter,
        tensor,
        image,
    } = picked_image_info;

    let depth_meter = depth_meter.map(|d| *d.0);

    ui.label(instance_path.to_string());

    if tensor.is_some() {
        if true {
            // Only show the `TensorData` component, to keep the hover UI small; see https://github.com/rerun-io/rerun/issues/3573
            use re_types::Loggable as _;
            let component_path = re_log_types::ComponentPath::new(
                instance_path.entity_path.clone(),
                re_types::components::TensorData::name(),
            );
            component_path.data_ui_recording(ctx, ui, UiLayout::List);
        } else {
            // Show it all, like we do for any other thing we hover
            instance_path.data_ui_recording(ctx, ui, UiLayout::List);
        }
    }

    let wh = if let Some(tensor) = &tensor {
        tensor.image_height_width_channels().map(|[h, w, _]| (w, h))
    } else if let Some(image) = &image {
        Some((image.resolution[0] as u64, image.resolution[1] as u64))
    } else {
        None
    };

    let Some((w, h)) = wh else {
        return;
    };

    ui.add_space(8.0);

    ui.horizontal(|ui| {
        let (w, h) = (w as f32, h as f32);

        if spatial_kind == SpatialSpaceViewKind::TwoD {
            let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(w, h));

            show_zoomed_image_region_area_outline(
                ui.ctx(),
                ui_clip_rect,
                egui::vec2(w, h),
                [coordinates[0] as _, coordinates[1] as _],
                space_from_ui.inverse().transform_rect(rect),
            );
        }

        let tensor_name = instance_path.to_string();

        let annotations = annotations.0.find(&instance_path.entity_path);

        if let Some(image) = &image {
            let tensor_stats = ctx.cache.entry(|c: &mut ImageStatsCache| c.entry(image));
            if let Some(render_ctx) = ctx.render_ctx {
                show_zoomed_image_region(
                    render_ctx,
                    ui,
                    image,
                    &tensor_stats,
                    &annotations,
                    meaning,
                    depth_meter,
                    &tensor_name,
                    [coordinates[0] as _, coordinates[1] as _],
                );
            }
        }
        if let Some(tensor) = &tensor {
            let tensor_stats = ctx
                .cache
                .entry(|c: &mut TensorStatsCache| c.entry(row_id, tensor));
            if let Some(render_ctx) = ctx.render_ctx {
                show_zoomed_tensor_region(
                    render_ctx,
                    ui,
                    row_id,
                    tensor,
                    &tensor_stats,
                    &annotations,
                    meaning,
                    depth_meter,
                    &tensor_name,
                    [coordinates[0] as _, coordinates[1] as _],
                    Some(colormap),
                );
            }
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
