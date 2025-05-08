use rerun::external::{
    egui,
    re_log_types::{EntityPath, Instance},
    re_renderer,
    re_viewer_context::{
        self, IdentifiedViewSystem, ViewContext, ViewContextCollection, ViewQuery,
        ViewSystemExecutionError, ViewSystemIdentifier, VisualizerQueryInfo, VisualizerSystem,
    },
};
use rerun::Component as _;

use crate::color_archetype::ColorArchetype;

/// Our view consist of single part which holds a list of egui colors for each entity path.
#[derive(Default)]
pub struct InstanceColorSystem {
    pub colors: Vec<(EntityPath, Vec<ColorWithInstance>)>,
}

pub struct ColorWithInstance {
    pub color: egui::Color32,
    pub instance: Instance,
}

impl IdentifiedViewSystem for InstanceColorSystem {
    fn identifier() -> ViewSystemIdentifier {
        "InstanceColor".into()
    }
}

impl VisualizerSystem for InstanceColorSystem {
    fn visualizer_query_info(&self) -> VisualizerQueryInfo {
        VisualizerQueryInfo::from_archetype::<ColorArchetype>()
    }

    /// Populates the visualizer with data from the store.
    fn execute(
        &mut self,
        ctx: &ViewContext<'_>,
        query: &ViewQuery<'_>,
        _context_systems: &ViewContextCollection,
    ) -> Result<Vec<re_renderer::QueueableDrawData>, ViewSystemExecutionError> {
        // For each entity in the view that should be displayed with the `InstanceColorSystem`…
        for data_result in query.iter_visible_data_results(Self::identifier()) {
            // TODO(#6889): This is an _interesting_ but really really strange example.
            // UI doesn't play nicely with it as it won't show anything when one of these color points is selected.

            // First gather all kinds of colors that are logged on this path.
            let recording_engine = ctx.recording_engine();
            let color_descriptors = recording_engine
                .store()
                .entity_component_descriptors_with_name(
                    &data_result.entity_path,
                    rerun::Color::name(),
                );

            // Query them from the cache.
            let results = ctx.recording_engine().cache().latest_at(
                &ctx.current_query(),
                &data_result.entity_path,
                color_descriptors.iter(),
            );

            // Collect all different kinds of colors that are returned from the cache.
            let colors = results
                .components
                .iter()
                .flat_map(|(descr, chunk)| chunk.iter_slices::<u32>(descr.component_name).flatten())
                .collect::<Vec<_>>();

            if colors.is_empty() {
                continue;
            }

            self.colors.push((
                data_result.entity_path.clone(),
                (0..)
                    .zip(colors)
                    .map(|(instance, color)| {
                        let [r, g, b, _] = rerun::Color::from_u32(*color).to_array();
                        ColorWithInstance {
                            color: egui::Color32::from_rgb(r, g, b),
                            instance: instance.into(),
                        }
                    })
                    .collect(),
            ));
        }

        // We're not using `re_renderer` here, so return an empty vector.
        // If you want to draw additional primitives here, you can emit re_renderer draw data here directly.
        Ok(Vec::new())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn fallback_provider(&self) -> &dyn re_viewer_context::ComponentFallbackProvider {
        self
    }
}

// Implements a `ComponentFallbackProvider` trait for the `InstanceColorSystem`.
// It is left empty here but could be used to provides fallback values for optional components in case they're missing.
use rerun::external::re_types;
re_viewer_context::impl_component_fallback_provider!(InstanceColorSystem => []);
