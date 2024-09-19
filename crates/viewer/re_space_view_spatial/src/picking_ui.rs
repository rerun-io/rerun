use egui::NumExt as _;

use re_data_ui::{
    item_ui, show_zoomed_image_region, show_zoomed_image_region_area_outline, DataUi as _,
};
use re_log_types::Instance;
use re_renderer::renderer::ColormappedTexture;
use re_ui::{
    list_item::{list_item_scope, PropertyContent},
    UiExt as _,
};
use re_viewer_context::{
    ImageStatsCache, Item, ItemSpaceContext, SpaceViewSystemExecutionError, UiLayout, ViewQuery,
    ViewerContext, VisualizerCollection,
};

use crate::{
    contexts::AnnotationSceneContext,
    picking::{PickableUiRect, PickingContext, PickingHitType},
    ui::SpatialSpaceViewState,
    view_kind::SpatialSpaceViewKind,
    visualizers::{iter_spatial_visualizer_data, CamerasVisualizer, DepthImageVisualizer},
    PickableRectSourceData, PickableTexturedRect,
};

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

        if hit.hit_type == PickingHitType::TexturedRect {
            // We don't support selecting pixels yet.
            instance_path.instance = Instance::ALL;
        }

        response = if let Some(picked_pixel) = get_pixel_picking_info(system_output, hit) {
            if let PickableRectSourceData::Image {
                depth_meter: Some(meter),
                image,
            } = &picked_pixel.source_data
            {
                let [x, y] = picked_pixel.pixel_coordinates;
                if let Some(raw_value) = image.get_xyc(x, y, 0) {
                    let raw_value = raw_value.as_f64();
                    let depth_in_meters = raw_value / *meter.0 as f64;
                    depth_at_pointer = Some(depth_in_meters as f32);
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
                            picking_context.camera_plane_from_ui,
                            annotations,
                            picked_pixel,
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
                    .pointer_in_camera_plane
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

/// If available, finds pixel info for a picking hit.
///
/// Returns `None` for error placeholder since we generally don't want to zoom into those.
fn get_pixel_picking_info(
    system_output: &re_viewer_context::SystemExecutionOutput,
    hit: &crate::picking::PickingRayHit,
) -> Option<PickedPixelInfo> {
    let depth_visualizer_output = system_output
        .view_systems
        .get::<DepthImageVisualizer>()
        .ok();

    if hit.hit_type == PickingHitType::TexturedRect {
        iter_pickable_rects(&system_output.view_systems)
            .find(|i| i.ent_path.hash() == hit.instance_path_hash.entity_path_hash)
            .and_then(|picked_rect| {
                if matches!(
                    picked_rect.source_data,
                    PickableRectSourceData::ErrorPlaceholder
                ) {
                    return None;
                }

                let pixel_coordinates = hit
                    .instance_path_hash
                    .instance
                    .to_2d_image_coordinate(picked_rect.resolution()[0]);

                Some(PickedPixelInfo {
                    source_data: picked_rect.source_data.clone(),
                    texture: picked_rect.textured_rect.colormapped_texture.clone(),
                    pixel_coordinates,
                })
            })
    } else if let Some((depth_image, depth_meter, texture)) =
        depth_visualizer_output.and_then(|depth_images| {
            depth_images
                .depth_cloud_entities
                .get(&hit.instance_path_hash.entity_path_hash)
        })
    {
        let pixel_coordinates = hit
            .instance_path_hash
            .instance
            .to_2d_image_coordinate(depth_image.width());
        Some(PickedPixelInfo {
            source_data: PickableRectSourceData::Image {
                image: depth_image.clone(),
                depth_meter: Some(*depth_meter),
            },
            texture: texture.clone(),
            pixel_coordinates,
        })
    } else {
        None
    }
}

struct PickedPixelInfo {
    source_data: PickableRectSourceData,
    texture: ColormappedTexture,
    pixel_coordinates: [u32; 2],
}

#[allow(clippy::too_many_arguments)]
fn image_hover_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    instance_path: &re_entity_db::InstancePath,
    spatial_kind: SpatialSpaceViewKind,
    ui_pan_and_zoom_from_ui: egui::emath::RectTransform,
    annotations: &AnnotationSceneContext,
    picked_pixel_info: PickedPixelInfo,
) {
    let PickedPixelInfo {
        source_data,
        texture,
        pixel_coordinates,
    } = picked_pixel_info;

    let depth_meter = match &source_data {
        PickableRectSourceData::Image { depth_meter, .. } => *depth_meter,
        PickableRectSourceData::Video { .. } => None,
        PickableRectSourceData::ErrorPlaceholder => {
            // No point in zooming into an error placeholder!
            return;
        }
    };

    let depth_meter = depth_meter.map(|d| *d.0);

    ui.label(instance_path.to_string());

    ui.add_space(8.0);

    ui.horizontal(|ui| {
        let [w, h] = texture.width_height();
        let (w, h) = (w as f32, h as f32);

        if spatial_kind == SpatialSpaceViewKind::TwoD {
            let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(w, h));

            show_zoomed_image_region_area_outline(
                ui.ctx(),
                *ui_pan_and_zoom_from_ui.from(),
                egui::vec2(w, h),
                [pixel_coordinates[0] as _, pixel_coordinates[1] as _],
                ui_pan_and_zoom_from_ui.inverse().transform_rect(rect),
            );
        }

        if let PickableRectSourceData::Image { image, .. } = &source_data {
            let debug_name = instance_path.to_string();
            let annotations = annotations.0.find(&instance_path.entity_path);
            let tensor_stats = ctx.cache.entry(|c: &mut ImageStatsCache| c.entry(image));

            if let Some(render_ctx) = ctx.render_ctx {
                show_zoomed_image_region(
                    render_ctx,
                    ui,
                    image,
                    &tensor_stats,
                    &annotations,
                    depth_meter,
                    &debug_name,
                    [pixel_coordinates[0] as _, pixel_coordinates[1] as _],
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
