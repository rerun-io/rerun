use egui::{Context, NumExt as _};
use re_data_ui::{item_ui, DataUi};
use re_log_types::EntityPath;
use re_space_view::suggest_space_view_for_each_entity;
use re_types::{
    blueprint::{
        archetypes::{MapBackground, MapZoom},
        components::MapProvider,
        components::ZoomLevel,
    },
    SpaceViewClassIdentifier, View,
};
use re_ui::list_item;
use re_viewer_context::{
    Item, SpaceViewClass, SpaceViewClassLayoutPriority, SpaceViewClassRegistryError, SpaceViewId,
    SpaceViewSpawnHeuristics, SpaceViewState, SpaceViewStateExt as _,
    SpaceViewSystemExecutionError, SpaceViewSystemRegistrator, SystemExecutionOutput, UiLayout,
    ViewQuery, ViewerContext,
};
use re_viewport_blueprint::ViewProperty;
use walkers::{HttpTiles, Map, MapMemory, Tiles};

use crate::map_overlays;
use crate::visualizers::geo_points::GeoPointsVisualizer;

#[derive(Default)]
pub struct MapSpaceViewState {
    tiles: Option<HttpTiles>,
    map_memory: MapMemory,
    selected_provider: MapProvider,
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
        egui::Frame::default().show(ui, |ui| {
            let mut picked_instance = None;

            let some_tiles_manager: Option<&mut dyn Tiles> = Some(tiles);
            let map_response = ui.add(
                Map::new(some_tiles_manager, map_memory, default_center_position).with_plugin(
                    geo_points_visualizer.plugin(ctx, query.space_view_id, &mut picked_instance),
                ),
            );

            if let Some(picked_instance) = picked_instance {
                map_response.clone().on_hover_ui_at_pointer(|ui| {
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
                    ))
                }
            } else if map_response.clicked() {
                // clicked elsewhere, select the view
                ctx.selection_state()
                    .set_selection(Item::SpaceView(query.space_view_id))
            }

            if map_response.double_clicked() {
                map_memory.follow_my_position();
                if let Some(zoom_level) = default_zoom_level {
                    let _ = map_memory.set_zoom(zoom_level);
                }
            }

            let map_rect = map_response.rect;
            map_overlays::zoom_buttons_overlay(ui, &map_rect, map_memory);
            map_overlays::acknowledgement_overlay(ui, &map_rect, &tiles.attribution());
        });

        //
        // Save Blueprint
        //

        if Some(map_memory.zoom()) != blueprint_zoom_level {
            map_zoom.save_blueprint_component(
                ctx,
                &ZoomLevel(re_types::datatypes::Float64(map_memory.zoom())),
            );
        }

        Ok(())
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
