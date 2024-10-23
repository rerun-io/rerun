use re_space_view::{DataResultQuery as _, RangeResultsExt as _};
use re_types::{
    archetypes::Points3D,
    components::{self, Color, Position3D, Radius},
    Loggable as _,
};
use re_viewer_context::{
    auto_color_for_entity_path, IdentifiedViewSystem, QueryContext, SpaceViewSystemExecutionError,
    TypedComponentFallbackProvider, ViewContext, ViewContextCollection, ViewQuery,
    VisualizerQueryInfo, VisualizerSystem,
};

// ---

#[derive(Debug, Clone)]
pub struct GeoPointEntry {
    /// Position.
    pub position: walkers::Position,

    /// Display radius in pixels
    //TODO(#7872): support for radius in meter
    pub radius: f32,

    /// Color.
    pub color: egui::Color32,
}

impl Default for GeoPointEntry {
    fn default() -> Self {
        Self {
            position: walkers::Position::from_lat_lon(51.4934, 0.),
            radius: 10.0,
            color: egui::Color32::RED,
        }
    }
}

/// A map scene, with entries on the map to render.
#[derive(Default)]
pub struct GeoPointsVisualizer {
    pub map_entries: Vec<GeoPointEntry>,
}

impl IdentifiedViewSystem for GeoPointsVisualizer {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "GeoPoints".into()
    }
}

impl VisualizerSystem for GeoPointsVisualizer {
    fn visualizer_query_info(&self) -> VisualizerQueryInfo {
        VisualizerQueryInfo::from_archetype::<Points3D>()
    }

    fn execute(
        &mut self,
        ctx: &ViewContext<'_>,
        view_query: &ViewQuery<'_>,
        _context_systems: &ViewContextCollection,
    ) -> Result<Vec<re_renderer::QueueableDrawData>, SpaceViewSystemExecutionError> {
        for data_result in view_query.iter_visible_data_results(ctx, Self::identifier()) {
            let results = data_result.query_archetype_with_history::<Points3D>(ctx, view_query);

            // gather all relevant chunks
            let timeline = view_query.timeline;
            let all_positions = results.iter_as(timeline, Position3D::name());
            let all_colors = results.iter_as(timeline, components::Color::name());
            let all_radii = results.iter_as(timeline, components::Radius::name());

            // default component values
            let default_color: Color =
                self.fallback_for(&ctx.query_context(data_result, &view_query.latest_at_query()));
            let default_radius: Radius =
                self.fallback_for(&ctx.query_context(data_result, &view_query.latest_at_query()));

            // iterate over each chunk and find all relevant component slices
            for (_index, position, color, radii) in re_query::range_zip_1x2(
                all_positions.component::<Position3D>(),
                all_colors.component::<Color>(),
                all_radii.component::<Radius>(),
            ) {
                // required component
                let position = position.as_slice();

                // optional components
                let color = color.as_ref().map(|c| c.as_slice()).unwrap_or(&[]);
                let radii = radii.as_ref().map(|r| r.as_slice()).unwrap_or(&[]);

                // optional components values to be used for instance clamping semantics
                let last_color = color.last().copied().unwrap_or(default_color);
                let last_radii = radii.last().copied().unwrap_or(default_radius);

                // iterate over all instances
                for (position, color, radius) in itertools::izip!(
                    position,
                    color.iter().chain(std::iter::repeat(&last_color)),
                    radii.iter().chain(std::iter::repeat(&last_radii)),
                ) {
                    self.map_entries.push(GeoPointEntry {
                        position: walkers::Position::from_lat_lon(
                            position.x() as f64,
                            position.y() as f64,
                        ),
                        //TODO(#7872): support for radius in meter
                        radius: radius.0.abs(),
                        color: color.0.into(),
                    });
                }
            }
        }

        Ok(Vec::new())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn fallback_provider(&self) -> &dyn re_viewer_context::ComponentFallbackProvider {
        self
    }
}

impl TypedComponentFallbackProvider<Color> for GeoPointsVisualizer {
    fn fallback_for(&self, ctx: &QueryContext<'_>) -> Color {
        auto_color_for_entity_path(ctx.target_entity_path)
    }
}

impl TypedComponentFallbackProvider<Radius> for GeoPointsVisualizer {
    fn fallback_for(&self, _ctx: &QueryContext<'_>) -> Radius {
        Radius::from(5.0)
    }
}

re_viewer_context::impl_component_fallback_provider!(GeoPointsVisualizer => [Color, Radius]);

impl GeoPointsVisualizer {
    /// Return a [`walkers::Plugin`] for this visualizer.
    pub fn plugin(&self) -> impl walkers::Plugin + '_ {
        GeoPointsPlugin {
            map_entries: &self.map_entries,
        }
    }
}

pub struct GeoPointsPlugin<'a> {
    pub map_entries: &'a Vec<GeoPointEntry>,
}

impl walkers::Plugin for GeoPointsPlugin<'_> {
    fn run(
        &mut self,
        _response: &egui::Response,
        painter: egui::Painter,
        projector: &walkers::Projector,
    ) {
        for entry in self.map_entries {
            // Project it into the position on the screen.
            let position = projector.project(entry.position).to_pos2();
            painter.circle_filled(position, entry.radius, entry.color);
        }
    }
}
