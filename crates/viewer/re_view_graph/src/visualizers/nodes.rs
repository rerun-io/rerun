use egui::Color32;
use re_chunk::LatestAtQuery;
use re_log_types::{EntityPath, Instance};
use re_query::{clamped_zip_2x4, range_zip_1x4};
use re_types::components::{Color, Radius, ShowLabels};
use re_types::{
    self, archetypes,
    components::{self},
    ArrowString, Component as _,
};
use re_view::{DataResultQuery, RangeResultsExt};
use re_viewer_context::{
    self, IdentifiedViewSystem, QueryContext, TypedComponentFallbackProvider, ViewContext,
    ViewContextCollection, ViewQuery, ViewSystemExecutionError, ViewSystemIdentifier,
    VisualizerQueryInfo, VisualizerSystem,
};

use crate::graph::NodeId;

#[derive(Default)]
pub struct NodeVisualizer {
    pub data: ahash::HashMap<EntityPath, NodeData>,
}

pub const FALLBACK_RADIUS: f32 = 4.0;

/// The label information of a [`re_types::archetypes::GraphNodes`].
#[derive(Clone)]
pub enum Label {
    Circle {
        /// Radius of the circle in world coordinates.
        radius: f32,
        color: Option<Color32>,
    },
    Text {
        text: ArrowString,
        color: Option<Color32>,
    },
}

/// A [`NodeInstance`] is the output of the [`NodeVisualizer`] and represents a single node in the graph.
#[derive(Clone)]
pub struct NodeInstance {
    pub instance_index: Instance,
    pub id: NodeId,
    pub position: Option<egui::Pos2>,
    pub label: Label,
}

pub struct NodeData {
    pub nodes: Vec<NodeInstance>,
}

impl IdentifiedViewSystem for NodeVisualizer {
    fn identifier() -> ViewSystemIdentifier {
        "GraphNodes".into()
    }
}

impl VisualizerSystem for NodeVisualizer {
    fn visualizer_query_info(&self) -> VisualizerQueryInfo {
        VisualizerQueryInfo::from_archetype::<archetypes::GraphNodes>()
    }

    /// Populates the scene part with data from the store.
    fn execute(
        &mut self,
        ctx: &ViewContext<'_>,
        query: &ViewQuery<'_>,
        _context_systems: &ViewContextCollection,
    ) -> Result<Vec<re_renderer::QueueableDrawData>, ViewSystemExecutionError> {
        let timeline_query = LatestAtQuery::new(query.timeline, query.latest_at);

        for data_result in query.iter_visible_data_results(ctx, Self::identifier()) {
            let results = data_result
                .latest_at_with_blueprint_resolved_data::<archetypes::GraphNodes>(
                    ctx,
                    &timeline_query,
                );

            let all_indexed_nodes = results.iter_as(query.timeline, components::GraphNode::name());
            let all_colors = results.iter_as(query.timeline, components::Color::name());
            let all_positions = results.iter_as(query.timeline, components::Position2D::name());
            let all_labels = results.iter_as(query.timeline, components::Text::name());
            let all_radii = results.iter_as(query.timeline, components::Radius::name());
            let show_label = results
                .get_mono::<components::ShowLabels>()
                .map_or(true, bool::from);

            let data = range_zip_1x4(
                // TODO(cmc): Provide a `iter_struct`.
                all_indexed_nodes.component_slow::<components::GraphNode>(),
                all_colors.primitive::<u32>(),
                all_positions.primitive_array::<2, f32>(),
                all_labels.string(),
                all_radii.primitive::<f32>(),
            );

            for (_index, nodes, colors, positions, labels, radii) in data {
                let nodes = clamped_zip_2x4(
                    nodes.iter(),
                    (0..).map(Instance::from),
                    colors.unwrap_or_default().iter().map(Option::Some),
                    Option::<&u32>::default,
                    positions
                        .unwrap_or_default()
                        .iter()
                        .copied()
                        .map(Option::Some),
                    Option::<[f32; 2]>::default,
                    labels.unwrap_or_default().iter().cloned().map(Option::Some),
                    Option::<ArrowString>::default,
                    radii.unwrap_or_default().iter().copied().map(Option::Some),
                    Option::<f32>::default,
                )
                .map(|(node, instance, color, position, label, radius)| {
                    let color = color.map(|&c| egui::Color32::from(Color::new(c)));
                    let label = match (label, show_label) {
                        (Some(label), true) => Label::Text {
                            text: label.clone(),
                            color,
                        },
                        (None, true) => Label::Text {
                            text: node.0 .0.clone(),
                            color,
                        },
                        _ => Label::Circle {
                            // Radius is negative for UI radii, but we don't handle this here.
                            radius: radius.unwrap_or(FALLBACK_RADIUS).abs(),
                            color,
                        },
                    };

                    NodeInstance {
                        instance_index: instance,
                        id: NodeId::from_entity_node(&data_result.entity_path, node),
                        position: position.map(|[x, y]| egui::Pos2::new(x, y)),
                        label,
                    }
                })
                .collect::<Vec<_>>();

                self.data
                    .insert(data_result.entity_path.clone(), NodeData { nodes });
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

impl TypedComponentFallbackProvider<ShowLabels> for NodeVisualizer {
    fn fallback_for(&self, _ctx: &QueryContext<'_>) -> ShowLabels {
        true.into()
    }
}

impl TypedComponentFallbackProvider<Radius> for NodeVisualizer {
    fn fallback_for(&self, _ctx: &QueryContext<'_>) -> Radius {
        FALLBACK_RADIUS.into()
    }
}

re_viewer_context::impl_component_fallback_provider!(NodeVisualizer => [ShowLabels, Radius]);
