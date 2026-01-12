use egui::Color32;
use re_chunk::LatestAtQuery;
use re_log_types::{EntityPath, Instance};
use re_query::{clamped_zip_2x4, range_zip_1x4};
use re_sdk_types::archetypes::GraphNodes;
use re_sdk_types::components::{
    Color, {self},
};
use re_sdk_types::{self, ArrowString, archetypes};
use re_view::{DataResultQuery as _, RangeResultsExt as _};
use re_viewer_context::{
    self, IdentifiedViewSystem, ViewContext, ViewContextCollection, ViewQuery,
    ViewSystemExecutionError, ViewSystemIdentifier, VisualizerExecutionOutput, VisualizerQueryInfo,
    VisualizerSystem,
};

use crate::graph::NodeId;

#[derive(Default)]
pub struct NodeVisualizer {
    pub data: ahash::HashMap<EntityPath, NodeData>,
}

pub const FALLBACK_RADIUS: f32 = 4.0;

/// The label information of a [`re_sdk_types::archetypes::GraphNodes`].
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

    /// Populates the visualizer with data from the store.
    fn execute(
        &mut self,
        ctx: &ViewContext<'_>,
        query: &ViewQuery<'_>,
        _context_systems: &ViewContextCollection,
    ) -> Result<VisualizerExecutionOutput, ViewSystemExecutionError> {
        let timeline_query = LatestAtQuery::new(query.timeline, query.latest_at);

        for (data_result, instruction) in query.iter_visualizer_instruction_for(Self::identifier())
        {
            let results = data_result
                .latest_at_with_blueprint_resolved_data::<archetypes::GraphNodes>(
                    ctx,
                    &timeline_query,
                    instruction,
                );

            let all_nodes =
                results.iter_as(query.timeline, GraphNodes::descriptor_node_ids().component);
            let all_colors =
                results.iter_as(query.timeline, GraphNodes::descriptor_colors().component);
            let all_positions =
                results.iter_as(query.timeline, GraphNodes::descriptor_positions().component);
            let all_labels =
                results.iter_as(query.timeline, GraphNodes::descriptor_labels().component);
            let all_radii =
                results.iter_as(query.timeline, GraphNodes::descriptor_radii().component);
            let show_label = results
                .get_mono::<components::ShowLabels>(GraphNodes::descriptor_show_labels().component)
                .is_none_or(bool::from);

            let data = range_zip_1x4(
                all_nodes.slice::<String>(),
                all_colors.slice::<u32>(),
                all_positions.slice::<[f32; 2]>(),
                all_labels.slice::<String>(),
                all_radii.slice::<f32>(),
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
                    let node = components::GraphNode(node.clone().into());
                    let color = color.map(|&c| egui::Color32::from(Color::new(c)));
                    let label = match (label, show_label) {
                        (Some(label), true) => Label::Text {
                            text: label.clone(),
                            color,
                        },
                        (None, true) => Label::Text {
                            text: node.0.0.clone(),
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
                        id: NodeId::from_entity_node(&data_result.entity_path, &node),
                        position: position.map(|[x, y]| egui::Pos2::new(x, y)),
                        label,
                    }
                })
                .collect::<Vec<_>>();

                self.data
                    .insert(data_result.entity_path.clone(), NodeData { nodes });
            }
        }

        Ok(VisualizerExecutionOutput::default())
    }
}
