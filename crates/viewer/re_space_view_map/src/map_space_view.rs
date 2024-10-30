use egui::{Context, NumExt as _};
use re_entity_db::InstancePathHash;
use re_renderer::OutlineConfig;
use walkers::{HttpTiles, Map, MapMemory, Tiles};

use re_data_ui::{item_ui, DataUi};
use re_log_types::{EntityPath, EntityPathHash, Instance};
use re_space_view::suggest_space_view_for_each_entity;
use re_types::{
    blueprint::{
        archetypes::{MapBackground, MapZoom},
        components::MapProvider,
        components::ZoomLevel,
    },
    SpaceViewClassIdentifier, View,
};
use re_ui::{list_item, ContextExt as _};
use re_viewer_context::{
    gpu_bridge, Item, SpaceViewClass, SpaceViewClassLayoutPriority, SpaceViewClassRegistryError,
    SpaceViewId, SpaceViewSpawnHeuristics, SpaceViewState, SpaceViewStateExt as _,
    SpaceViewSystemExecutionError, SpaceViewSystemRegistrator, SystemExecutionOutput, UiLayout,
    ViewQuery, ViewerContext,
};
use re_viewport_blueprint::ViewProperty;

use crate::map_overlays;
use crate::visualizers::geo_points::GeoPointsVisualizer;

#[derive(Default)]
pub struct MapSpaceViewState {
    tiles: Option<HttpTiles>,
    map_memory: MapMemory,
    selected_provider: MapProvider,

    last_gpu_picking_result: Option<InstancePathHash>,
}

impl MapSpaceViewState {
    // This method ensures that tiles is initialized and returns mutable references to tiles and map_memory.
    pub fn ensure_and_get_mut_refs(
        &mut self,
        ctx: &egui::Context,
    ) -> Result<(&mut HttpTiles, &mut MapMemory), SpaceViewSystemExecutionError> {
        if self.tiles.is_none() {
            let tiles = get_tile_manager(self.selected_provider, ctx);
            self.tiles = Some(tiles);
        }

        // Now that tiles is guaranteed to be Some, unwrap is safe here.
        let tiles_ref = self
            .tiles
            .as_mut()
            .ok_or(SpaceViewSystemExecutionError::MapTilesError)?;
        Ok((tiles_ref, &mut self.map_memory))
    }
}

impl SpaceViewState for MapSpaceViewState {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

#[derive(Default)]
pub struct MapSpaceView;

type ViewType = re_types::blueprint::views::MapView;

impl SpaceViewClass for MapSpaceView {
    fn identifier() -> SpaceViewClassIdentifier {
        ViewType::identifier()
    }

    fn display_name(&self) -> &'static str {
        "Map"
    }

    fn icon(&self) -> &'static re_ui::Icon {
        &re_ui::icons::SPACE_VIEW_MAP
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
        system_registry: &mut SpaceViewSystemRegistrator<'_>,
    ) -> Result<(), SpaceViewClassRegistryError> {
        system_registry.register_visualizer::<GeoPointsVisualizer>()
    }

    fn new_state(&self) -> Box<dyn SpaceViewState> {
        Box::<MapSpaceViewState>::new(MapSpaceViewState::default())
    }

    fn preferred_tile_aspect_ratio(&self, _state: &dyn SpaceViewState) -> Option<f32> {
        // Prefer a square tile if possible.
        Some(1.0)
    }

    fn layout_priority(&self) -> SpaceViewClassLayoutPriority {
        SpaceViewClassLayoutPriority::Low
    }

    fn spawn_heuristics(&self, ctx: &ViewerContext<'_>) -> SpaceViewSpawnHeuristics {
        suggest_space_view_for_each_entity::<GeoPointsVisualizer>(ctx, self)
    }

    fn selection_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut dyn SpaceViewState,
        _space_origin: &EntityPath,
        space_view_id: SpaceViewId,
    ) -> Result<(), SpaceViewSystemExecutionError> {
        re_ui::list_item::list_item_scope(ui, "map_selection_ui", |ui| {
            re_space_view::view_property_ui::<MapZoom>(ctx, ui, space_view_id, self, state);
            re_space_view::view_property_ui::<MapBackground>(ctx, ui, space_view_id, self, state);
        });

        Ok(())
    }

    fn ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut dyn SpaceViewState,
        query: &ViewQuery<'_>,
        system_output: SystemExecutionOutput,
    ) -> Result<(), SpaceViewSystemExecutionError> {
        let state = state.downcast_mut::<MapSpaceViewState>()?;
        let map_background = ViewProperty::from_archetype::<MapBackground>(
            ctx.blueprint_db(),
            ctx.blueprint_query,
            query.space_view_id,
        );

        let map_zoom = ViewProperty::from_archetype::<MapZoom>(
            ctx.blueprint_db(),
            ctx.blueprint_query,
            query.space_view_id,
        );

        let geo_points_visualizer = system_output.view_systems.get::<GeoPointsVisualizer>()?;

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

        let span = geo_points_visualizer.span();

        let default_center_position = span
            .as_ref()
            .map(|span| span.center())
            .unwrap_or(walkers::Position::from_lat_lon(59.319224, 18.075514)); // Rerun HQ

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

        let (tiles, map_memory) = match state.ensure_and_get_mut_refs(ui.ctx()) {
            Ok(refs) => refs,
            Err(err) => return Err(err),
        };

        let mut picked_instance = None;

        let some_tiles_manager: Option<&mut dyn Tiles> = Some(tiles);
        let mut map_response = ui.add(
            Map::new(some_tiles_manager, map_memory, default_center_position).with_plugin(
                geo_points_visualizer.plugin(ctx, query.space_view_id, &mut picked_instance),
            ),
        );
        let map_rect = map_response.rect;

        if let Some(picked_instance) = picked_instance {
            map_response = map_response.on_hover_ui_at_pointer(|ui| {
                list_item::list_item_scope(ui, "map_hover", |ui| {
                    item_ui::instance_path_button(
                        ctx,
                        &query.latest_at_query(),
                        ctx.recording(),
                        ui,
                        Some(query.space_view_id),
                        &picked_instance.instance_path,
                    );
                    picked_instance
                        .instance_path
                        .data_ui_recording(ctx, ui, UiLayout::Tooltip);
                });
            });

            ctx.select_hovered_on_click(
                &map_response,
                Item::DataResult(query.space_view_id, picked_instance.instance_path.clone()),
            );

            // double click selects the entire entity
            if map_response.double_clicked() {
                // Select the entire entity
                ctx.selection_state().set_selection(Item::DataResult(
                    query.space_view_id,
                    picked_instance.instance_path.entity_path.clone().into(),
                ));
            }
        } else if map_response.clicked() {
            // clicked elsewhere, select the view
            ctx.selection_state()
                .set_selection(Item::SpaceView(query.space_view_id));
        }

        if map_response.double_clicked() {
            map_memory.follow_my_position();
            if let Some(zoom_level) = default_zoom_level {
                let _ = map_memory.set_zoom(zoom_level);
            }
        }

        map_overlays::zoom_buttons_overlay(ui, &map_rect, map_memory);
        map_overlays::acknowledgement_overlay(ui, &map_rect, &tiles.attribution());

        //
        // Save Blueprint
        //

        if Some(map_memory.zoom()) != blueprint_zoom_level {
            map_zoom.save_blueprint_component(
                ctx,
                &ZoomLevel(re_types::datatypes::Float64(map_memory.zoom())),
            );
        }

        // ---------------------------------------------------------------------------

        let Some(render_ctx) = ctx.render_ctx else {
            return Err(SpaceViewSystemExecutionError::NoRenderContextError);
        };
        let painter = ui.painter();

        let resolution_in_pixel =
            gpu_bridge::viewport_resolution_in_pixels(map_rect, ui.ctx().pixels_per_point());

        let mut view_builder = re_renderer::ViewBuilder::new(
            // TODO: make this a util
            render_ctx,
            re_renderer::view_builder::TargetConfiguration {
                name: "MapView".into(),
                resolution_in_pixel,

                // Camera looking at a ui coordinate world.
                view_from_world: re_math::IsoTransform::from_translation(-glam::vec3(
                    map_rect.left(),
                    map_rect.top(),
                    0.0,
                )),
                projection_from_view: re_renderer::view_builder::Projection::Orthographic {
                    camera_mode:
                        re_renderer::view_builder::OrthographicCameraMode::TopLeftCornerAndExtendZ,
                    vertical_world_size: map_rect.height(),
                    far_plane_distance: 100.0,
                },
                // No transform after view/projection needed.
                viewport_transformation: re_renderer::RectTransform::IDENTITY,

                pixels_per_point: ui.ctx().pixels_per_point(),
                //outline_config: query
                //    .highlights
                //    .any_outlines()
                //    .then(|| outline_config(ui.ctx())),
                outline_config: Some(outline_config(ui.ctx())),
            },
        );

        // TODO: populate view builder with the things it should draw.
        let mut points = re_renderer::PointCloudBuilder::new(render_ctx);
        points
            .batch("Antoine")
            .picking_object_id(re_renderer::PickingLayerObjectId(321)) // Entity path.
            //.outline_mask_ids(outline_mask_ids) // Entire thing
            .push_additional_outline_mask_ids_for_range(
                0..1, // Instances 0 to 1, that is order they're added.
                re_renderer::OutlineMaskPreference::some(0, 1),
            )
            .add_points_2d(
                &[glam::vec3(map_rect.center().x, map_rect.center().y, 0.0)],
                &[re_renderer::Size::ONE_UI_POINT * 10.0],
                &[re_renderer::Color32::LIGHT_RED],
                &[re_renderer::PickingLayerInstanceId(1234)], // picking instance id == index
            );
        view_builder.queue_draw(points.into_draw_data()?);

        // ---------------------------------------------------------------------------

        let picking_readback_identifier = 123; // TODO: should be unique per view (NOT view type)

        if let Some(pointer_in_ui) = map_response.hover_pos() {
            let mut pointer_in_pixel = pointer_in_ui.to_vec2();
            pointer_in_pixel -= map_rect.min.to_vec2();
            pointer_in_pixel *= ui.ctx().pixels_per_point();

            let picking_result = picking_gpu(
                render_ctx,
                picking_readback_identifier,
                glam::vec2(pointer_in_pixel.x, pointer_in_pixel.y),
                &mut state.last_gpu_picking_result,
            );
            re_log::debug!("Picking result: {picking_result:?}"); // TODO:

            // ------
            // TODO: more wonky c&p

            /// Radius in which cursor interactions may snap to the nearest object even if the cursor
            /// does not hover it directly.
            ///
            /// Note that this needs to be scaled when zooming is applied by the virtual->visible ui rect transform.
            pub const UI_INTERACTION_RADIUS: f32 = 5.0;

            let picking_rect_size = UI_INTERACTION_RADIUS * ui.ctx().pixels_per_point();
            // Make the picking rect bigger than necessary so we can use it to counter-act delays.
            // (by the time the picking rectangle is read back, the cursor may have moved on).
            let picking_rect_size = (picking_rect_size * 2.0)
                .ceil()
                .at_least(8.0)
                .at_most(128.0) as u32;
            // ------

            view_builder
                .schedule_picking_rect(
                    render_ctx,
                    re_renderer::RectInt::from_middle_and_extent(
                        glam::ivec2(pointer_in_pixel.x as _, pointer_in_pixel.y as _),
                        glam::uvec2(picking_rect_size, picking_rect_size),
                    ),
                    picking_readback_identifier,
                    (),
                    true, // TODO: debug overlay, put to app settings.
                )
                .expect("antoine do something");
        } else {
            // TODO: should we keep flushing out the gpu picking results? Does spatial view do this?

            state.last_gpu_picking_result = None;
        }

        // ---------------------------------------------------------------------------

        painter.add(gpu_bridge::new_renderer_callback(
            view_builder,
            painter.clip_rect(),
            re_renderer::Rgba::TRANSPARENT,
        ));

        Ok(())
    }
}

// TODO: we have sinned here. this is a c&p from re_space_view_spatial. we should make this a util.
pub fn outline_config(gui_ctx: &egui::Context) -> re_renderer::OutlineConfig {
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

fn get_tile_manager(provider: MapProvider, egui_ctx: &Context) -> HttpTiles {
    let mapbox_access_token = std::env::var("RERUN_MAPBOX_ACCESS_TOKEN").unwrap_or_default();

    match provider {
        MapProvider::OpenStreetMap => {
            HttpTiles::new(walkers::sources::OpenStreetMap, egui_ctx.clone())
        }
        MapProvider::MapboxStreets => HttpTiles::new(
            walkers::sources::Mapbox {
                style: walkers::sources::MapboxStyle::Streets,
                access_token: mapbox_access_token.clone(),
                high_resolution: false,
            },
            egui_ctx.clone(),
        ),
        MapProvider::MapboxDark => HttpTiles::new(
            walkers::sources::Mapbox {
                style: walkers::sources::MapboxStyle::Dark,
                access_token: mapbox_access_token.clone(),
                high_resolution: false,
            },
            egui_ctx.clone(),
        ),
        MapProvider::MapboxSatellite => HttpTiles::new(
            walkers::sources::Mapbox {
                style: walkers::sources::MapboxStyle::Satellite,
                access_token: mapbox_access_token.clone(),
                high_resolution: true,
            },
            egui_ctx.clone(),
        ),
    }
}

re_viewer_context::impl_component_fallback_provider!(MapSpaceView => []);

// TODO:
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
        // TODO: the block inside this particular branch is so reusable, it should probably live on re_renderer! (on picking result?)

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
        let mut picked_on_picking_rect = glam::Vec2::ZERO;
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
                    picked_on_picking_rect = current_pos_on_picking_rect;
                    closest_rect_distance_sq = distance_sq;
                    picked_id = *id;
                }
            }
        }

        let new_result = if picked_id == re_renderer::PickingLayerId::default() {
            // Nothing found.
            None
        } else {
            Some(instance_path_hash_from_picking_layer_id(picked_id))
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

// TODO: again this is verbatim copy pasted from re_space_view_spatial. we should make this a util.
#[inline]
pub fn instance_path_hash_from_picking_layer_id(
    value: re_renderer::PickingLayerId,
) -> InstancePathHash {
    InstancePathHash {
        entity_path_hash: EntityPathHash::from_u64(value.object.0),
        // `PickingLayerId` uses `u64::MAX` to mean "hover and/or select all instances".
        instance: if value.instance.0 == u64::MAX {
            Instance::ALL
        } else {
            Instance::from(value.instance.0)
        },
    }
}
