use re_data_store::LatestAtQuery;
use re_types::{archetypes::Points3D, components, ComponentName};
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
            let resolver = ctx.recording().resolver();

            let results = ctx.recording().latest_at(
                &timeline_query,
                &data_result.entity_path,
                [
                    ComponentName::from("rerun.components.Position3D"),
                    ComponentName::from("rerun.components.Color"),
                    ComponentName::from("rerun.components.Radius"),
                ],
            );

            let positions = results
                .get_slice::<components::Position3D>(resolver)
                .unwrap_or_default()
                .iter()
                .map(|pos| Position::from_lat_lon(pos.x() as f64, pos.y() as f64))
                .collect::<Vec<_>>();

            let radii = results
                .get_slice::<components::Radius>(resolver)
                .unwrap_or_default();

            let colors = results
                .get_slice::<components::Color>(resolver)
                .unwrap_or_default();

            let it_positions = positions.into_iter();
            let it_radii = radii.iter().map(Some).chain(std::iter::repeat(None));
            let it_colors = colors.iter().map(Some).chain(std::iter::repeat(None));

            for ((pos, rad), col) in it_positions.zip(it_radii).zip(it_colors) {
                self.map_entries.push(MapEntry {
                    position: pos,
                    radii: rad.copied(),
                    color: col.copied(),
                });
            }
        }

        Ok(Vec::new())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_fallback_provider(&self) -> &dyn re_viewer_context::ComponentFallbackProvider {
        self
    }
}

re_viewer_context::impl_component_fallback_provider!(MapVisualizerSystem => []);
