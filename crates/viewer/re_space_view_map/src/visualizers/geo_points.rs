use re_space_view::{DataResultQuery as _, RangeResultsExt as _};
use re_types::{
    archetypes::Points3D,
    components::{self, Color, Position3D, Radius},
    Loggable as _,
};
use re_viewer_context::{
    IdentifiedViewSystem, SpaceViewSystemExecutionError, ViewContext, ViewContextCollection,
    ViewQuery, VisualizerQueryInfo, VisualizerSystem,
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

            let timeline = view_query.timeline;
            let all_positions = results.iter_as(timeline, Position3D::name());
            let all_colors = results.iter_as(timeline, components::Color::name());
            let all_radii = results.iter_as(timeline, components::Radius::name());

            let mut last_color = None;
            let mut last_radius = None;

            for (_index, position, color, radii) in re_query::range_zip_1x2(
                all_positions.component::<Position3D>(),
                all_colors.component::<Color>(),
                all_radii.component::<Radius>(),
            ) {
                last_color = color
                    .as_ref()
                    .map(|c| c.as_slice().last())
                    .flatten()
                    .cloned()
                    .or(last_color);
                last_radius = radii
                    .as_ref()
                    .map(|r| r.as_slice().last())
                    .flatten()
                    .cloned()
                    .or(last_radius);

                for (idx, pos) in position.as_slice().iter().enumerate() {
                    let color = color
                        .as_ref()
                        .and_then(|c| c.as_slice().get(idx))
                        .or(last_color.as_ref())
                        .map(|c| {
                            let c = c.0.to_array();
                            egui::Color32::from_rgba_premultiplied(c[0], c[1], c[2], c[3])
                        })
                        .unwrap_or(egui::Color32::RED); //TODO: fallback provider

                    let radius = radii
                        .as_ref()
                        .and_then(|r| r.as_slice().get(idx))
                        .or(last_radius.as_ref())
                        .map(|r| {
                            //TODO(#7872): support for radius in meter
                            r.0.abs()
                        })
                        .unwrap_or(5.0); //TODO: fallback provider

                    self.map_entries.push(GeoPointEntry {
                        position: walkers::Position::from_lat_lon(pos.x() as f64, pos.y() as f64),
                        radius,
                        color,
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

re_viewer_context::impl_component_fallback_provider!(GeoPointsVisualizer => []);

impl GeoPointsVisualizer {
    /// Return a [`walkers::Plugin`] for this visualizer.
    pub fn plugin<'a>(&'a self) -> impl walkers::Plugin + 'a {
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
