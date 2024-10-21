use re_chunk_store::LatestAtQuery;
use re_space_view::DataResultQuery;
use re_types::{
    archetypes::Points3D,
    components::{self, Position3D},
};
use re_viewer_context::{
    IdentifiedViewSystem, SpaceViewSystemExecutionError, ViewContext, ViewContextCollection,
    ViewQuery, VisualizerQueryInfo, VisualizerSystem,
};
use walkers::Position;

// ---

#[derive(Debug, Clone)]
pub struct MapEntry {
    pub position: Position,
    pub radii: Option<components::Radius>,
    pub color: Option<components::Color>,
}

impl Default for MapEntry {
    fn default() -> Self {
        Self {
            position: Position::from_lat_lon(51.4934, 0.),
            radii: None,
            color: None,
        }
    }
}

/// A map scene, with entries on the map to render.
#[derive(Default)]
pub struct MapVisualizerSystem {
    pub map_entries: Vec<MapEntry>,
}

impl IdentifiedViewSystem for MapVisualizerSystem {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "Map".into()
    }
}

impl VisualizerSystem for MapVisualizerSystem {
    fn visualizer_query_info(&self) -> VisualizerQueryInfo {
        VisualizerQueryInfo::from_archetype::<Points3D>()
    }

    fn execute(
        &mut self,
        ctx: &ViewContext<'_>,
        view_query: &ViewQuery<'_>,
        _context_systems: &ViewContextCollection,
    ) -> Result<Vec<re_renderer::QueueableDrawData>, SpaceViewSystemExecutionError> {
        let timeline_query = LatestAtQuery::new(view_query.timeline, view_query.latest_at);

        for data_result in view_query.iter_visible_data_results(ctx, Self::identifier()) {
            // let resolver = ctx.recording().resolver();

            let results = data_result
                .latest_at_with_blueprint_resolved_data::<Points3D>(ctx, &timeline_query);

            let Some(position) = results.get_required_mono::<Position3D>() else {
                continue;
            };

            let color = results.get_mono_with_fallback::<components::Color>();
            let radii = results.get_mono_with_fallback::<components::Radius>();

            self.map_entries.push(MapEntry {
                position: Position::from_lat_lon(position.x() as f64, position.y() as f64),
                radii: Some(radii),
                color: Some(color),
            });
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

re_viewer_context::impl_component_fallback_provider!(MapVisualizerSystem => []);
