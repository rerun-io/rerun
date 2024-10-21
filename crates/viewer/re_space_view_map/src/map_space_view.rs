use re_space_view::controls::{
    ASPECT_SCROLL_MODIFIER, HORIZONTAL_SCROLL_MODIFIER, SELECTION_RECT_ZOOM_BUTTON,
    ZOOM_SCROLL_MODIFIER,
};
use re_types::blueprint::{
    archetypes::MapOptions,
    components::{Secret, ZoomLevel},
};
use re_ui::{ModifiersMarkdown, MouseButtonMarkdown};
use re_viewer_context::TypedComponentFallbackProvider;
use re_viewport_blueprint::ViewProperty;

use {
    egui::{self, Color32, Context},
    re_log_types::EntityPath,
    re_space_view::suggest_space_view_for_each_entity,
    re_types::blueprint::components::MapProvider,
    re_types::{
        components::{Color, Radius},
        SpaceViewClassIdentifier, View,
    },
    re_viewer_context::{
        SpaceViewClass, SpaceViewClassLayoutPriority, SpaceViewClassRegistryError, SpaceViewId,
        SpaceViewSpawnHeuristics, SpaceViewState, SpaceViewStateExt as _,
        SpaceViewSystemExecutionError, SpaceViewSystemRegistrator, SystemExecutionOutput,
        ViewQuery, ViewerContext,
    },
    walkers::{HttpTiles, Map, MapMemory, Plugin, Tiles},
};

use crate::map_visualizer_system::{MapEntry, MapVisualizerSystem};
use crate::map_windows;

// walkers plugin to visualize points on a Map
pub struct PositionsOnMap {
    positions: Vec<MapEntry>,
}

impl Plugin for PositionsOnMap {
    fn run(
        &mut self,
        _response: &egui::Response,
        painter: egui::Painter,
        projector: &walkers::Projector,
    ) {
        for entry in &self.positions {
            // Position of the point we want to put our shapes.
            let position = entry.position;

            // Project it into the position on the screen.
            let position = projector.project(position).to_pos2();

            // Radius of the circle
            let radius = f32::from(
                *entry
                    .radii
                    .unwrap_or(Radius(re_types::datatypes::Float32(10.))),
            );

            // Color of the circle
            let color = entry.color.unwrap_or(Color::new(Color32::RED));

            painter.circle_filled(position, radius, color);
        }
    }
}

#[derive(Default)]
pub struct MapSpaceViewState {
    tiles: Option<HttpTiles>,
    map_memory: MapMemory,
    selected_provider: MapProvider,
    access_token: String,
}

impl MapSpaceViewState {
    // This method ensures that tiles is initialized and returns mutable references to tiles and map_memory.
    pub fn ensure_and_get_mut_refs(
        &mut self,
        ctx: &egui::Context,
    ) -> Result<(&mut HttpTiles, &mut MapMemory), SpaceViewSystemExecutionError> {
        if self.tiles.is_none() {
            let tiles = get_tile_manager(self.selected_provider, &self.access_token, ctx);
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

    fn help_markdown(&self, egui_ctx: &egui::Context) -> String {
        format!(
            "# Map view

Displays a Position3D on a map.

## Navigation controls

- Pan by dragging, or scroll (+{horizontal_scroll_modifier} for horizontal).
- Zoom with pinch gesture or scroll + {zoom_scroll_modifier}.
- Scroll + {aspect_scroll_modifier} to zoom only the temporal axis while holding the y-range fixed.
- Drag with the {selection_rect_zoom_button} to zoom in/out using a selection.
- Double-click to reset the view.",
            horizontal_scroll_modifier = ModifiersMarkdown(HORIZONTAL_SCROLL_MODIFIER, egui_ctx),
            zoom_scroll_modifier = ModifiersMarkdown(ZOOM_SCROLL_MODIFIER, egui_ctx),
            aspect_scroll_modifier = ModifiersMarkdown(ASPECT_SCROLL_MODIFIER, egui_ctx),
            selection_rect_zoom_button = MouseButtonMarkdown(SELECTION_RECT_ZOOM_BUTTON),
        )
    }

    fn on_register(
        &self,
        system_registry: &mut SpaceViewSystemRegistrator<'_>,
    ) -> Result<(), SpaceViewClassRegistryError> {
        system_registry.register_visualizer::<MapVisualizerSystem>()
    }

    fn new_state(&self) -> Box<dyn SpaceViewState> {
        Box::<MapSpaceViewState>::new(MapSpaceViewState {
            tiles: None,
            map_memory: MapMemory::default(),
            selected_provider: MapProvider::default(),
            access_token: std::env::var("MAPBOX_ACCESS_TOKEN").unwrap_or_default(),
        })
    }

    fn preferred_tile_aspect_ratio(&self, _state: &dyn SpaceViewState) -> Option<f32> {
        // Prefer a square tile if possible.
        Some(1.0)
    }

    fn layout_priority(&self) -> SpaceViewClassLayoutPriority {
        SpaceViewClassLayoutPriority::Low
    }

    fn spawn_heuristics(&self, ctx: &ViewerContext<'_>) -> SpaceViewSpawnHeuristics {
        suggest_space_view_for_each_entity::<MapVisualizerSystem>(ctx, self)
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
            re_space_view::view_property_ui::<re_types::blueprint::archetypes::MapOptions>(
                ctx,
                ui,
                space_view_id,
                self,
                state,
            );
        });

        // "Center and follow" button to reset view following mode after interacting
        // with the map.
        let map_state = state.downcast_mut::<MapSpaceViewState>()?;
        ui.horizontal(|ui| {
            let is_detached = map_state.map_memory.detached().is_none();

            if !is_detached && ui.button("Center and follow positions").clicked() {
                map_state.map_memory.follow_my_position();
            }
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
        let map_viz_system = system_output.view_systems.get::<MapVisualizerSystem>()?;

        let blueprint_db = ctx.blueprint_db();
        let view_id = query.space_view_id;
        let map_options =
            ViewProperty::from_archetype::<MapOptions>(blueprint_db, ctx.blueprint_query, view_id);
        let map_provider = map_options.component_or_fallback::<MapProvider>(ctx, self, state)?;
        let zoom_level = map_options
            .component_or_fallback::<ZoomLevel>(ctx, self, state)?
            .0;
        let access_token = map_options
            .component_or_fallback::<Secret>(ctx, self, state)?
            .0
            .to_string();

        if state.map_memory.set_zoom(*zoom_level).is_err() {
            re_log::warn!(
                "Failed to set zoom level for map. Zoom level should be between zero and 22"
            );
        };

        // if state changed let's update it from the blueprint
        if state.selected_provider != map_provider || access_token != state.access_token {
            state.tiles = None;
            state.selected_provider = map_provider;
            state.access_token = access_token;
        }

        let (tiles, map_memory) = match state.ensure_and_get_mut_refs(ui.ctx()) {
            Ok(refs) => refs,
            Err(err) => return Err(err),
        };

        egui::Frame::default().show(ui, |ui| {
            let some_tiles_manager: Option<&mut dyn Tiles> = Some(tiles);
            let map_widget = ui.add(
                Map::new(
                    some_tiles_manager,
                    map_memory,
                    map_viz_system
                        .map_entries
                        .first()
                        .unwrap_or(&MapEntry::default())
                        .position,
                )
                .with_plugin(PositionsOnMap {
                    positions: map_viz_system.map_entries.clone(),
                }),
            );

            map_widget.double_clicked().then(|| {
                map_memory.follow_my_position();
            });

            let map_pos = map_widget.rect;
            let window_id = query.space_view_id.uuid().to_string();
            map_windows::zoom(ui, &window_id, &map_pos, map_memory);
            map_windows::acknowledge(ui, &window_id, &map_pos, tiles.attribution());

            // update blueprint if zoom level changed from ui
            if map_memory.zoom() != *zoom_level {
                map_options.save_blueprint_component(
                    ctx,
                    &ZoomLevel(re_types::datatypes::Float32(map_memory.zoom())),
                );
            }
        });
        Ok(())
    }
}

fn get_tile_manager(provider: MapProvider, access_token: &str, egui_ctx: &Context) -> HttpTiles {
    match provider {
        MapProvider::OpenStreetMap => {
            HttpTiles::new(walkers::sources::OpenStreetMap, egui_ctx.clone())
        }
        MapProvider::MapboxStreets => HttpTiles::new(
            walkers::sources::Mapbox {
                style: walkers::sources::MapboxStyle::Streets,
                access_token: access_token.to_owned(),
                high_resolution: false,
            },
            egui_ctx.clone(),
        ),
        MapProvider::MapboxDark => HttpTiles::new(
            walkers::sources::Mapbox {
                style: walkers::sources::MapboxStyle::Dark,
                access_token: access_token.to_owned(),
                high_resolution: false,
            },
            egui_ctx.clone(),
        ),
        MapProvider::MapboxSatellite => HttpTiles::new(
            walkers::sources::Mapbox {
                style: walkers::sources::MapboxStyle::Satellite,
                access_token: access_token.to_owned(),
                high_resolution: true,
            },
            egui_ctx.clone(),
        ),
    }
}

impl TypedComponentFallbackProvider<ZoomLevel> for MapSpaceView {
    fn fallback_for(&self, _ctx: &re_viewer_context::QueryContext<'_>) -> ZoomLevel {
        // default zoom level is 16.
        16.0.into()
    }
}

impl TypedComponentFallbackProvider<Secret> for MapSpaceView {
    fn fallback_for(&self, _ctx: &re_viewer_context::QueryContext<'_>) -> Secret {
        // default zoom level is 16.
        std::env::var("MAPBOX_ACCESS_TOKEN")
            .unwrap_or_default()
            .into()
    }
}

re_viewer_context::impl_component_fallback_provider!(MapSpaceView => [ZoomLevel, Secret]);
