use re_log_types::Instance;
use re_viewer::external::{
    egui::Color32,
    re_chunk::{ChunkComponentIterItem, LatestAtQuery},
    re_log_types::EntityPath,
    re_query::{clamped_zip_2x2, range_zip_1x3},
    re_renderer,
    re_space_view::{DataResultQuery, RangeResultsExt},
    re_types::{
        self, archetypes,
        components::{self},
        datatypes, ArrowString, Loggable as _,
    },
    re_viewer_context::{
        self, IdentifiedViewSystem, SpaceViewSystemExecutionError, ViewContext,
        ViewContextCollection, ViewQuery, ViewSystemIdentifier, VisualizerQueryInfo,
        VisualizerSystem,
    },
};

/// Our space view consist of single part which holds a list of egui colors for each entity path.
#[derive(Default)]
pub struct GraphNodeVisualizer {
    pub(crate) data: Vec<GraphNodeVisualizerData>,
}

pub(crate) struct GraphNodeVisualizerData {
    pub entity_path: EntityPath,
    node_ids: ChunkComponentIterItem<components::GraphNodeId>,

    // Clamped
    colors: ChunkComponentIterItem<components::Color>,
    labels: Vec<ArrowString>,

    // Non-repeated
    show_labels: Option<components::ShowLabels>,
}

pub(crate) struct NodeInstance<'a> {
    pub node_id: &'a datatypes::GraphNodeId,
    pub entity_path: &'a EntityPath,
    pub instance: Instance,
    pub label: Option<&'a ArrowString>,
    pub color: Option<Color32>,
}

impl GraphNodeVisualizerData {
    pub(crate) fn nodes(&self) -> impl Iterator<Item = NodeInstance> {
        clamped_zip_2x2(
            self.node_ids.iter(),
            (0..).map(Instance::from),
            self.colors.iter().map(Option::Some),
            Option::<&components::Color>::default,
            self.labels.iter().map(|l| {
                if self.show_labels.is_some() {
                    Some(l)
                } else {
                    None
                }
            }),
            Option::<&ArrowString>::default,
        )
        .map(move |(node_id, instance, color, label)| NodeInstance {
            entity_path: &self.entity_path,
            node_id,
            instance,
            color: color.map(|c| Color32::from(c.0)),
            label,
        })
    }
}

impl IdentifiedViewSystem for GraphNodeVisualizer {
    fn identifier() -> ViewSystemIdentifier {
        "GraphNodes".into()
    }
}

impl VisualizerSystem for GraphNodeVisualizer {
    fn visualizer_query_info(&self) -> VisualizerQueryInfo {
        VisualizerQueryInfo::from_archetype::<archetypes::GraphNodes>()
    }

    /// Populates the scene part with data from the store.
    fn execute(
        &mut self,
        ctx: &ViewContext<'_>,
        query: &ViewQuery<'_>,
        _context_systems: &ViewContextCollection,
    ) -> Result<Vec<re_renderer::QueueableDrawData>, SpaceViewSystemExecutionError> {
        let timeline_query = LatestAtQuery::new(query.timeline, query.latest_at);

        for data_result in query.iter_visible_data_results(ctx, Self::identifier()) {
            let results = data_result
                .latest_at_with_blueprint_resolved_data::<archetypes::GraphNodes>(
                    ctx,
                    &timeline_query,
                );

            let all_indexed_nodes =
                results.iter_as(query.timeline, components::GraphNodeId::name());
            let all_colors = results.iter_as(query.timeline, components::Color::name());
            let all_labels = results.iter_as(query.timeline, components::Text::name());
            let all_show_labels = results.iter_as(query.timeline, components::ShowLabels::name());

            let data = range_zip_1x3(
                all_indexed_nodes.component::<components::GraphNodeId>(),
                all_colors.component::<components::Color>(),
                all_labels.string(),
                all_show_labels.component::<components::ShowLabels>(),
            );

            for (_index, node_ids, colors, labels, show_labels) in data {
                self.data.push(GraphNodeVisualizerData {
                    entity_path: data_result.entity_path.clone(),
                    node_ids,
                    colors: colors.unwrap_or_default(),
                    labels: labels.unwrap_or_default(),
                    show_labels: show_labels.unwrap_or_default().first().copied(),
                });
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

re_viewer_context::impl_component_fallback_provider!(GraphNodeVisualizer => []);
