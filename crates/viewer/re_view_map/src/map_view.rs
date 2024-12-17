use egui::{Context, NumExt as _, Rect, Response};
use re_view::AnnotationSceneContext;
use walkers::{HttpTiles, Map, MapMemory, Tiles};

use re_data_ui::{item_ui, DataUi};
use re_entity_db::InstancePathHash;
use re_log_types::EntityPath;
use re_renderer::{RenderContext, ViewBuilder};
use re_types::{
    blueprint::{
        archetypes::{MapBackground, MapZoom},
        components::MapProvider,
        components::ZoomLevel,
    },
    View, ViewClassIdentifier,
};
use re_ui::list_item;
use re_viewer_context::{
    gpu_bridge, IdentifiedViewSystem as _, Item, SystemExecutionOutput, UiLayout, ViewClass,
    ViewClassLayoutPriority, ViewClassRegistryError, ViewHighlights, ViewId, ViewQuery,
    ViewSpawnHeuristics, ViewState, ViewStateExt as _, ViewSystemExecutionError,
    ViewSystemRegistrator, ViewerContext,
};
use re_viewport_blueprint::ViewProperty;

use crate::map_overlays;
use crate::visualizers::{update_span, GeoLineStringsVisualizer, GeoPointsVisualizer};

pub struct MapViewState {
    tiles: Option<HttpTiles>,
    map_memory: MapMemory,
    selected_provider: MapProvider,

    last_center_position: walkers::Position,

    /// Because `re_renderer` can have varying, multiple frames of delay, we must keep track of the
    /// last picked results for when picking results is not available on a given frame.
    last_gpu_picking_result: Option<InstancePathHash>,
}

impl Default for MapViewState {
    fn default() -> Self {
        Self {
            tiles: None,
            map_memory: Default::default(),
            selected_provider: Default::default(),

            // default to Rerun HQ whenever we have no data (either now or historically) to provide
            // a better location
            last_center_position: walkers::Position::from_lat_lon(59.319224, 18.075514),
            last_gpu_picking_result: None,
        }
    }
}

impl MapViewState {
    // This method ensures that tiles is initialized and returns mutable references to tiles and map_memory.
    pub fn ensure_and_get_mut_refs(
        &mut self,
        ctx: &ViewerContext<'_>,
        egui_ctx: &egui::Context,
    ) -> Result<(&mut HttpTiles, &mut MapMemory), ViewSystemExecutionError> {
        if self.tiles.is_none() {
            let tiles = get_tile_manager(ctx, self.selected_provider, egui_ctx);
            self.tiles = Some(tiles);
        }

        // Now that tiles is guaranteed to be Some, unwrap is safe here.
        let tiles_ref = self
            .tiles
            .as_mut()
            .ok_or(ViewSystemExecutionError::MapTilesError)?;
        Ok((tiles_ref, &mut self.map_memory))
    }
}

impl ViewState for MapViewState {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

#[derive(Default)]
pub struct MapView;

type ViewType = re_types::blueprint::views::MapView;

impl ViewClass for MapView {
    fn identifier() -> ViewClassIdentifier {
        ViewType::identifier()
    }

    fn display_name(&self) -> &'static str {
        "Map"
    }

    fn icon(&self) -> &'static re_ui::Icon {
        &re_ui::icons::VIEW_MAP
    }

    fn help_markdown(&self, _egui_ctx: &egui::Context) -> String {
        "# Map view

Displays geospatial primitives on a map.

## Navigation controls

- Pan by dragging.
- Zoom with pinch gesture.
- Double-click to reset the view."
            .to_owned()
    }

    fn on_register(
        &self,
        system_registry: &mut ViewSystemRegistrator<'_>,
    ) -> Result<(), ViewClassRegistryError> {
        system_registry.register_visualizer::<GeoPointsVisualizer>()?;
        system_registry.register_visualizer::<GeoLineStringsVisualizer>()?;

        system_registry.register_context_system::<AnnotationSceneContext>()?;

        Ok(())
    }

    fn new_state(&self) -> Box<dyn ViewState> {
        Box::<MapViewState>::new(MapViewState::default())
    }

    fn preferred_tile_aspect_ratio(&self, _state: &dyn ViewState) -> Option<f32> {
        // Prefer a square tile if possible.
        Some(1.0)
    }

    fn layout_priority(&self) -> ViewClassLayoutPriority {
        ViewClassLayoutPriority::default()
    }

    fn supports_visible_time_range(&self) -> bool {
        true
    }

    fn spawn_heuristics(&self, ctx: &ViewerContext<'_>) -> ViewSpawnHeuristics {
        re_tracing::profile_function!();

        // Spawn a single map view at the root if any geospatial entity exists.
        let any_map_entity = [
            GeoPointsVisualizer::identifier(),
            GeoLineStringsVisualizer::identifier(),
        ]
        .iter()
        .any(|system_id| {
            ctx.indicated_entities_per_visualizer
                .get(system_id)
                .is_some_and(|indicated_entities| !indicated_entities.is_empty())
        });

        if any_map_entity {
            ViewSpawnHeuristics::root()
        } else {
            ViewSpawnHeuristics::default()
        }
    }

    fn selection_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut dyn ViewState,
        _space_origin: &EntityPath,
        view_id: ViewId,
    ) -> Result<(), ViewSystemExecutionError> {
        re_ui::list_item::list_item_scope(ui, "map_selection_ui", |ui| {
            re_view::view_property_ui::<MapZoom>(ctx, ui, view_id, self, state);
            re_view::view_property_ui::<MapBackground>(ctx, ui, view_id, self, state);
        });

        Ok(())
    }

    fn ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut dyn ViewState,
        query: &ViewQuery<'_>,
        system_output: SystemExecutionOutput,
    ) -> Result<(), ViewSystemExecutionError> {
        let state = state.downcast_mut::<MapViewState>()?;
        let map_background = ViewProperty::from_archetype::<MapBackground>(
            ctx.blueprint_db(),
            ctx.blueprint_query,
            query.view_id,
        );

        let map_zoom = ViewProperty::from_archetype::<MapZoom>(
            ctx.blueprint_db(),
            ctx.blueprint_query,
            query.view_id,
        );

        let geo_points_visualizer = system_output.view_systems.get::<GeoPointsVisualizer>()?;
        let geo_line_strings_visualizers = system_output
            .view_systems
            .get::<GeoLineStringsVisualizer>()?;

        //
        // Map Provider
        //

        let map_provider = map_background.component_or_fallback::<MapProvider>(ctx, self, state)?;
        if state.selected_provider != map_provider {
            state.tiles = None;
            state.selected_provider = map_provider;
        }

        //
        // Pan/Zoom handling
        //

        // Rationale:
        // - `walkers` has an auto vs. manual pan state, switching to the latter upon
        //   user interaction. We let it keep track of that state.
        // - The tracked location is the center of the lat/lon span of the geo objects.
        // - When unset in the blueprint, the zoom level is computed from the geo objects and
        //   saved as is.
        // - Zoom computation: if multiple objects, fit them on screen, otherwise use 16.0.
        //
        // TODO(ab): show in UI and save in blueprint the auto vs. manual pan state (may require
        // changes in walkers
        // TODO(#7884): support more elaborate auto-pan/zoom modes.

        let mut span = None;
        update_span(&mut span, geo_points_visualizer.span());
        update_span(&mut span, geo_line_strings_visualizers.span());

        if let Some(span) = &span {
            state.last_center_position = span.center();
        }
        let default_center_position = state.last_center_position;

        let blueprint_zoom_level = map_zoom
            .component_or_empty::<ZoomLevel>()?
            .map(|zoom| **zoom);
        let default_zoom_level = span.and_then(|span| {
            span.zoom_for_screen_size(
                (ui.available_size() - egui::vec2(15.0, 15.0)).at_least(egui::Vec2::ZERO),
            )
        });
        let zoom_level = blueprint_zoom_level.or(default_zoom_level).unwrap_or(16.0);

        if state.map_memory.set_zoom(zoom_level).is_err() {
            //TODO(ab): we need a better handling of this, but requires upstream work (including
            //accepting higher zoom level and using lower-resolution tiles.
            re_log::debug!(
                "Zoom level {zoom_level} rejected by walkers (probably means that it is not \
                supported by the configured map provider)"
            );
        };

        //
        // Map UI
        //

        let (tiles, map_memory) = match state.ensure_and_get_mut_refs(ctx, ui.ctx()) {
            Ok(refs) => refs,
            Err(err) => return Err(err),
        };
        let attribution = tiles.attribution();

        let some_tiles_manager: Option<&mut dyn Tiles> = Some(tiles);
        let map_response = ui.add(Map::new(
            some_tiles_manager,
            map_memory,
            default_center_position,
        ));
        let map_rect = map_response.rect;
        let projector = walkers::Projector::new(map_rect, map_memory, default_center_position);

        if map_response.double_clicked() {
            map_memory.follow_my_position();
            if let Some(zoom_level) = default_zoom_level {
                let _ = map_memory.set_zoom(zoom_level);
            }
        }

        //
        // Save Blueprint
        //

        if Some(map_memory.zoom()) != blueprint_zoom_level {
            map_zoom.save_blueprint_component(
                ctx,
                &ZoomLevel(re_types::datatypes::Float64(map_memory.zoom())),
            );
        }

        //
        // Draw all objects using re_renderer
        //

        let Some(render_ctx) = ctx.render_ctx else {
            return Err(ViewSystemExecutionError::NoRenderContextError);
        };

        let mut view_builder =
            create_view_builder(render_ctx, ui.ctx(), map_rect, &query.highlights);

        geo_line_strings_visualizers.queue_draw_data(
            render_ctx,
            &mut view_builder,
            &projector,
            &query.highlights,
        )?;
        geo_points_visualizer.queue_draw_data(
            render_ctx,
            &mut view_builder,
            &projector,
            &query.highlights,
        )?;

        handle_picking_and_ui_interactions(
            ctx,
            render_ctx,
            ui.ctx(),
            &mut view_builder,
            query,
            state,
            map_response,
            map_rect,
        )?;

        ui.painter().add(gpu_bridge::new_renderer_callback(
            view_builder,
            map_rect,
            re_renderer::Rgba::TRANSPARENT,
        ));

        //
        // Attribution overlay
        //

        map_overlays::acknowledgement_overlay(ui, &map_rect, &attribution);

        Ok(())
    }
}

/// Create a view builder mapped to the provided rectangle.
///
/// The scene coordinates are 1:1 mapped to egui UI points.
//TODO(ab): this utility potentially has more general usefulness.
fn create_view_builder(
    render_ctx: &RenderContext,
    egui_ctx: &egui::Context,
    view_rect: Rect,
    highlights: &ViewHighlights,
) -> ViewBuilder {
    let pixels_per_point = egui_ctx.pixels_per_point();
    let resolution_in_pixel =
        gpu_bridge::viewport_resolution_in_pixels(view_rect, pixels_per_point);

    re_renderer::ViewBuilder::new(
        render_ctx,
        re_renderer::view_builder::TargetConfiguration {
            name: "MapView".into(),
            resolution_in_pixel,

            // Camera looking at a ui coordinate world.
            view_from_world: re_math::IsoTransform::from_translation(-glam::vec3(
                view_rect.left(),
                view_rect.top(),
                0.0,
            )),
            projection_from_view: re_renderer::view_builder::Projection::Orthographic {
                camera_mode:
                    re_renderer::view_builder::OrthographicCameraMode::TopLeftCornerAndExtendZ,
                vertical_world_size: view_rect.height(),
                far_plane_distance: 100.0,
            },
            // No transform after view/projection needed.
            viewport_transformation: re_renderer::RectTransform::IDENTITY,
            pixels_per_point,
            outline_config: highlights
                .any_outlines()
                .then(|| re_view::outline_config(egui_ctx)),

            // Make sure the map in the background is not completely overwritten
            blend_with_background: true,
        },
    )
}

/// Handle picking and related ui interactions.
#[allow(clippy::too_many_arguments)]
fn handle_picking_and_ui_interactions(
    ctx: &ViewerContext<'_>,
    render_ctx: &RenderContext,
    egui_ctx: &egui::Context,
    view_builder: &mut ViewBuilder,
    query: &ViewQuery<'_>,
    state: &mut MapViewState,
    map_response: Response,
    map_rect: Rect,
) -> Result<(), ViewSystemExecutionError> {
    let picking_readback_identifier = query.view_id.hash();

    if let Some(pointer_in_ui) = map_response.hover_pos() {
        let pixels_per_point = egui_ctx.pixels_per_point();
        let mut pointer_in_pixel = pointer_in_ui.to_vec2();
        pointer_in_pixel -= map_rect.min.to_vec2();
        pointer_in_pixel *= pixels_per_point;

        let picking_result = picking_gpu(
            render_ctx,
            picking_readback_identifier,
            glam::vec2(pointer_in_pixel.x, pointer_in_pixel.y),
            &mut state.last_gpu_picking_result,
        );

        handle_ui_interactions(ctx, query, map_response, picking_result);

        // TODO(ab, andreas): this part is copy-pasted-modified from spatial view and should be factored as an utility

        /// Radius in which cursor interactions may snap to the nearest object even if the cursor
        /// does not hover it directly.
        ///
        /// Note that this needs to be scaled when zooming is applied by the virtual->visible ui rect transform.
        pub const UI_INTERACTION_RADIUS: f32 = 5.0;

        let picking_rect_size = UI_INTERACTION_RADIUS * pixels_per_point;
        // Make the picking rect bigger than necessary so we can use it to counter-act delays.
        // (by the time the picking rectangle is read back, the cursor may have moved on).
        let picking_rect_size = (picking_rect_size * 2.0)
            .ceil()
            .at_least(8.0)
            .at_most(128.0) as u32;
        // ------

        view_builder.schedule_picking_rect(
            render_ctx,
            re_renderer::RectInt::from_middle_and_extent(
                glam::ivec2(pointer_in_pixel.x as _, pointer_in_pixel.y as _),
                glam::uvec2(picking_rect_size, picking_rect_size),
            ),
            picking_readback_identifier,
            (),
            ctx.app_options.show_picking_debug_overlay,
        )?;
    } else {
        // TODO(andreas): should we keep flushing out the gpu picking results? Does spatial view do this?
        state.last_gpu_picking_result = None;
    }
    Ok(())
}

/// Handle all UI interactions based on the currently picked instance (if any).
fn handle_ui_interactions(
    ctx: &ViewerContext<'_>,
    query: &ViewQuery<'_>,
    mut map_response: Response,
    picked_instance: Option<InstancePathHash>,
) {
    if let Some(instance_path) = picked_instance.and_then(|hash| hash.resolve(ctx.recording())) {
        map_response = map_response.on_hover_ui_at_pointer(|ui| {
            list_item::list_item_scope(ui, "map_hover", |ui| {
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
        });

        ctx.handle_select_hover_drag_interactions(
            &map_response,
            Item::DataResult(query.view_id, instance_path.clone()),
            false,
        );

        // double click selects the entire entity
        if map_response.double_clicked() {
            // Select the entire entity
            ctx.selection_state().set_selection(Item::DataResult(
                query.view_id,
                instance_path.entity_path.clone().into(),
            ));
        }
    } else if map_response.clicked() {
        // clicked elsewhere, select the view
        ctx.selection_state()
            .set_selection(Item::View(query.view_id));
    } else if map_response.hovered() {
        ctx.selection_state().set_hovered(Item::View(query.view_id));
    }
}

/// Return http options for tile downloads.
///
/// On native targets, it configures a cache directory.
fn http_options(_ctx: &ViewerContext<'_>) -> walkers::HttpOptions {
    #[cfg(not(target_arch = "wasm32"))]
    let options = walkers::HttpOptions {
        cache: _ctx.app_options.cache_subdirectory("map_view"),
        ..Default::default()
    };

    #[cfg(target_arch = "wasm32")]
    let options = Default::default();

    options
}

fn get_tile_manager(
    ctx: &ViewerContext<'_>,
    provider: MapProvider,
    egui_ctx: &Context,
) -> HttpTiles {
    let mapbox_access_token = ctx.app_options.mapbox_access_token().unwrap_or_default();

    let options = http_options(ctx);

    match provider {
        MapProvider::OpenStreetMap => {
            HttpTiles::with_options(walkers::sources::OpenStreetMap, options, egui_ctx.clone())
        }
        MapProvider::MapboxStreets => HttpTiles::with_options(
            walkers::sources::Mapbox {
                style: walkers::sources::MapboxStyle::Streets,
                access_token: mapbox_access_token.clone(),
                high_resolution: false,
            },
            options,
            egui_ctx.clone(),
        ),
        MapProvider::MapboxDark => HttpTiles::with_options(
            walkers::sources::Mapbox {
                style: walkers::sources::MapboxStyle::Dark,
                access_token: mapbox_access_token.clone(),
                high_resolution: false,
            },
            options,
            egui_ctx.clone(),
        ),
        MapProvider::MapboxSatellite => HttpTiles::with_options(
            walkers::sources::Mapbox {
                style: walkers::sources::MapboxStyle::Satellite,
                access_token: mapbox_access_token.clone(),
                high_resolution: true,
            },
            options,
            egui_ctx.clone(),
        ),
    }
}

re_viewer_context::impl_component_fallback_provider!(MapView => []);

// TODO(ab, andreas): this is a partial copy past of re_view_spatial::picking_gpu. Should be
// turned into a utility function.
fn picking_gpu(
    render_ctx: &re_renderer::RenderContext,
    gpu_readback_identifier: u64,
    pointer_in_pixel: glam::Vec2,
    last_gpu_picking_result: &mut Option<InstancePathHash>,
) -> Option<InstancePathHash> {
    re_tracing::profile_function!();

    // Only look at newest available result, discard everything else.
    let mut gpu_picking_result = None;
    while let Some(picking_result) = re_renderer::PickingLayerProcessor::next_readback_result::<()>(
        render_ctx,
        gpu_readback_identifier,
    ) {
        gpu_picking_result = Some(picking_result);
    }

    if let Some(gpu_picking_result) = gpu_picking_result {
        // TODO(ab, andreas): the block inside this particular branch is so reusable, it should probably live on re_renderer! (on picking result?)

        // First, figure out where on the rect the cursor is by now.
        // (for simplicity, we assume the screen hasn't been resized)
        let pointer_on_picking_rect = pointer_in_pixel - gpu_picking_result.rect.min.as_vec2();
        // The cursor might have moved outside of the rect. Clamp it back in.
        let pointer_on_picking_rect = pointer_on_picking_rect.clamp(
            glam::Vec2::ZERO,
            (gpu_picking_result.rect.extent - glam::UVec2::ONE).as_vec2(),
        );

        // Find closest non-zero pixel to the cursor.
        let mut picked_id = re_renderer::PickingLayerId::default();
        let mut closest_rect_distance_sq = f32::INFINITY;

        for (i, id) in gpu_picking_result.picking_id_data.iter().enumerate() {
            if id.object.0 != 0 {
                let current_pos_on_picking_rect = glam::uvec2(
                    i as u32 % gpu_picking_result.rect.extent.x,
                    i as u32 / gpu_picking_result.rect.extent.x,
                )
                .as_vec2()
                    + glam::vec2(0.5, 0.5); // Use pixel center for distances.
                let distance_sq =
                    current_pos_on_picking_rect.distance_squared(pointer_on_picking_rect);
                if distance_sq < closest_rect_distance_sq {
                    closest_rect_distance_sq = distance_sq;
                    picked_id = *id;
                }
            }
        }

        let new_result = if picked_id == re_renderer::PickingLayerId::default() {
            // Nothing found.
            None
        } else {
            Some(re_view::instance_path_hash_from_picking_layer_id(picked_id))
        };

        *last_gpu_picking_result = new_result;
        new_result
    } else {
        // It is possible that some frames we don't get a picking result and the frame after we get several.
        // We need to cache the last picking result and use it until we get a new one or the mouse leaves the screen.
        // (Andreas: On my mac this *actually* happens in very simple scenes, I get occasional frames with 0 and then with 2 picking results!)
        *last_gpu_picking_result
    }
}
