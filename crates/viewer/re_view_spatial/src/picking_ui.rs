use egui::NumExt as _;

use re_data_ui::{item_ui, DataUi as _};
use re_log_types::Instance;
use re_ui::{
    list_item::{list_item_scope, PropertyContent},
    UiExt as _,
};
use re_view::AnnotationSceneContext;
use re_viewer_context::{
    Item, ItemContext, UiLayout, ViewQuery, ViewSystemExecutionError, ViewerContext,
    VisualizerCollection,
};

use crate::{
    picking::{PickableUiRect, PickingContext, PickingHitType},
    picking_ui_pixel::{textured_rect_hover_ui, PickedPixelInfo},
    ui::SpatialViewState,
    view_kind::SpatialViewKind,
    visualizers::{CamerasVisualizer, DepthImageVisualizer, SpatialViewVisualizerData},
    PickableRectSourceData, PickableTexturedRect,
};

#[allow(clippy::too_many_arguments)]
pub fn picking(
    ctx: &ViewerContext<'_>,
    picking_context: &PickingContext,
    ui: &egui::Ui,
    mut response: egui::Response,
    view_builder: &mut re_renderer::view_builder::ViewBuilder,
    state: &mut SpatialViewState,
    system_output: &re_viewer_context::SystemExecutionOutput,
    ui_rects: &[PickableUiRect],
    query: &ViewQuery<'_>,
    spatial_kind: SpatialViewKind,
) -> Result<egui::Response, ViewSystemExecutionError> {
    re_tracing::profile_function!();

    if ui.ctx().dragged_id().is_some() {
        state.previous_picking_result = None;
        return Ok(response);
    }

    let picking_rect_size = PickingContext::UI_INTERACTION_RADIUS * ui.ctx().pixels_per_point();
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
        query.view_id.gpu_readback_id(),
        (),
        ctx.app_options.show_picking_debug_overlay,
    );

    let annotations = system_output
        .context_systems
        .get::<AnnotationSceneContext>()?;

    let picking_result = picking_context.pick(
        ctx.render_ctx,
        query.view_id.gpu_readback_id(),
        &state.previous_picking_result,
        iter_pickable_rects(&system_output.view_systems),
        ui_rects,
    );
    state.previous_picking_result = Some(picking_result.clone());

    let mut hovered_image_items = Vec::new();
    let mut hovered_non_image_items = Vec::new();

    // Depth at pointer used for projecting rays from a hovered 2D view to corresponding 3D view(s).
    // TODO(#1818): Depth at pointer only works for depth images so far.
    let mut depth_at_pointer = None;

    // We iterate front-to-back, putting foreground hits on top, like layers in Photoshop:
    for (hit_idx, hit) in picking_result.hits.iter().enumerate() {
        let Some(mut instance_path) = hit.instance_path_hash.resolve(ctx.recording()) else {
            // Entity no longer exists in db.
            continue;
        };

        if response.double_clicked() {
            // Select entire entity on double-click:
            instance_path.instance = Instance::ALL;
        }

        let query_result = ctx.lookup_query_result(query.view_id);
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
                        textured_rect_hover_ui(
                            ctx,
                            ui,
                            &instance_path,
                            query,
                            spatial_kind,
                            picking_context.camera_plane_from_ui,
                            annotations,
                            picked_pixel,
                            hit_idx as _,
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
                        Some(query.view_id),
                        &instance_path,
                    );
                    instance_path.data_ui_recording(ctx, ui, UiLayout::Tooltip);
                });
            })
        };

        let item = Item::DataResult(query.view_id, instance_path.clone());

        if hit.hit_type == PickingHitType::TexturedRect {
            hovered_image_items.push(item);
        } else {
            hovered_non_image_items.push(item);
        }
    }

    let hovered_items: Vec<Item> = {
        // Due to how our picking works, if we are hovering a point on top of an RGB and segmentation image,
        // we are actually hovering all three things (RGB, segmentation, point).
        // For the hovering preview (handled above) this is desierable: we want to zoom in on the
        // underlying image(s), even if the mouse slips over a point or some other geometric primitive.
        // However, when clicking we assume the users wants to only select the top-most thing.
        //
        // So we apply the following logic: if the hovered items are a mix of images and non-images,
        // then we only select the non-images on click.

        if !hovered_non_image_items.is_empty() {
            hovered_non_image_items
        } else if !hovered_image_items.is_empty() {
            hovered_image_items
        } else {
            // If we aren't hovering anything, we are hovering the view itself.
            vec![Item::View(query.view_id)]
        }
    };

    // Associate the hovered space with the first item in the hovered item list.
    // If we were to add several, views might render unnecessary additional hints.
    // TODO(andreas): Should there be context if no item is hovered at all? There's no usecase for that today it seems.
    let mut hovered_items = hovered_items
        .into_iter()
        .map(|item| (item, None))
        .collect::<Vec<_>>();

    if let Some((_, context)) = hovered_items.first_mut() {
        *context = Some(match spatial_kind {
            SpatialViewKind::TwoD => ItemContext::TwoD {
                space_2d: query.space_origin.clone(),
                pos: picking_context
                    .pointer_in_camera_plane
                    .extend(depth_at_pointer.unwrap_or(f32::INFINITY)),
            },
            SpatialViewKind::ThreeD => {
                let hovered_point = picking_result.space_position();
                let cameras_visualizer_output =
                    system_output.view_systems.get::<CamerasVisualizer>()?;

                ItemContext::ThreeD {
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

    ctx.handle_select_hover_drag_interactions(&response, hovered_items.into_iter(), false);

    Ok(response)
}

fn iter_pickable_rects(
    visualizers: &VisualizerCollection,
) -> impl Iterator<Item = &PickableTexturedRect> {
    visualizers
        .iter_visualizer_data::<SpatialViewVisualizerData>()
        .flat_map(|data| data.pickable_rects.iter())
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
