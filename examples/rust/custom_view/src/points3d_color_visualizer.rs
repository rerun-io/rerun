use rerun::external::egui;
use rerun::external::re_log_types::{EntityPath, Instance};
use rerun::external::re_view::{DataResultQuery, RangeResultsExt};
use rerun::external::re_viewer_context::{
    IdentifiedViewSystem, RequiredComponents, ViewContext, ViewContextCollection, ViewQuery,
    ViewSystemExecutionError, ViewSystemIdentifier, VisualizerExecutionOutput, VisualizerQueryInfo,
    VisualizerSystem,
};

/// Our view consist of single part which holds a list of egui colors for each entity path.
#[derive(Default)]
pub struct Points3DColorVisualizer {
    pub colors: Vec<(EntityPath, Vec<ColorWithInstance>)>,
}

pub struct ColorWithInstance {
    pub color: egui::Color32,
    pub instance: Instance,
}

impl IdentifiedViewSystem for Points3DColorVisualizer {
    fn identifier() -> ViewSystemIdentifier {
        "Points3DColorVisualizer".into()
    }
}

impl VisualizerSystem for Points3DColorVisualizer {
    fn visualizer_query_info(&self) -> VisualizerQueryInfo {
        // Usually, visualizers are closely tied to archetypes.
        // However, here we're adding a visualizer that queries only parts of an existing archetype.
        if false {
            // This is what it looks like to query all the fields of an archetype.
            VisualizerQueryInfo::from_archetype::<rerun::Points3D>()
        } else {
            // Instead, our custom query here is solely interested in Points3D's colors.
            VisualizerQueryInfo {
                relevant_archetype: Default::default(),
                required: RequiredComponents::AllComponents(
                    std::iter::once(rerun::Points3D::descriptor_colors().component).collect(),
                ),
                queried: std::iter::once(rerun::Points3D::descriptor_colors()).collect(),
            }
        }
    }

    /// Populates the visualizer with data from the store.
    fn execute(
        &mut self,
        ctx: &ViewContext<'_>,
        query: &ViewQuery<'_>,
        _context_systems: &ViewContextCollection,
    ) -> Result<VisualizerExecutionOutput, ViewSystemExecutionError> {
        // For each entity in the view that should be displayed with the `InstanceColorSystem`…
        for data_result in query.iter_visible_data_results(Self::identifier()) {
            // Query components while taking into account blueprint overrides
            // and visible history if enabled.
            let results = data_result.query_components_with_history(
                ctx,
                query,
                [rerun::Points3D::descriptor_colors().component],
            );

            // From the query result, get all the color arrays as `[u32]` slices.
            // For latest-at queries should be only a single slice`,
            // but if visible history is enabled, there might be several!
            let colors_per_time = results.iter_as(
                query.timeline,
                rerun::Points3D::descriptor_colors().component,
            );
            let color_slices_per_time = colors_per_time.slice::<u32>();

            // Collect all different kinds of colors that are returned from the cache.
            let mut colors_for_entity = Vec::new();
            for ((_time, _row_id), colors_slice) in color_slices_per_time {
                for (instance, color) in (0..).zip(colors_slice) {
                    let [r, g, b, _] = rerun::Color::from_u32(*color).to_array();
                    colors_for_entity.push(ColorWithInstance {
                        #[expect(clippy::disallowed_methods)] // This is not a hard-coded color.
                        color: egui::Color32::from_rgb(r, g, b),
                        instance: instance.into(),
                    });
                }
            }

            if !colors_for_entity.is_empty() {
                self.colors
                    .push((data_result.entity_path.clone(), colors_for_entity));
            }
        }

        // We're not using `re_renderer` here, so return an empty vector.
        // If you want to draw additional primitives here, you can emit re_renderer draw data here directly,
        // but your custom view's `ui` implementation has to set up an re_renderer output for this.
        Ok(VisualizerExecutionOutput::default())
    }
}
