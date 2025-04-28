use rerun::external::{
    re_renderer, re_types, re_view_spatial,
    re_viewer_context::{
        self, IdentifiedViewSystem, ViewContext, ViewContextCollection, ViewQuery,
        ViewSystemExecutionError, ViewSystemIdentifier, VisualizerQueryInfo, VisualizerSystem,
    },
};

use crate::{custom_archetype::Custom, custom_renderer::CustomDrawData};

#[derive(Default)]
pub struct CustomVisualizer {}

impl IdentifiedViewSystem for CustomVisualizer {
    fn identifier() -> ViewSystemIdentifier {
        "Custom".into()
    }
}

impl VisualizerSystem for CustomVisualizer {
    fn visualizer_query_info(&self) -> VisualizerQueryInfo {
        VisualizerQueryInfo::from_archetype::<Custom>()
    }

    fn execute(
        &mut self,
        ctx: &ViewContext<'_>,
        query: &ViewQuery<'_>,
        context_systems: &ViewContextCollection,
    ) -> Result<Vec<re_renderer::QueueableDrawData>, ViewSystemExecutionError> {
        let transforms = context_systems.get::<re_view_spatial::TransformTreeContext>()?;
        let render_ctx = ctx.render_ctx();

        let mut draw_data = CustomDrawData::new(render_ctx);

        for data_result in query.iter_visible_data_results(Self::identifier()) {
            let ent_path = &data_result.entity_path;
            let Some(transform_info) = transforms.transform_info_for_entity(ent_path.hash()) else {
                continue; // No valid transform info for this entity.
            };

            // TODO: handle component instances etc.

            for transform in &transform_info.reference_from_instances {
                draw_data.add(render_ctx, *transform, &ent_path.to_string());
            }
        }

        Ok(vec![draw_data.into()])
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn fallback_provider(&self) -> &dyn re_viewer_context::ComponentFallbackProvider {
        self
    }
}

// Implements a `ComponentFallbackProvider` trait for the `CustomVisualizer`.
// It is left empty here but could be used to provides fallback values for optional components in case they're missing.
re_viewer_context::impl_component_fallback_provider!(CustomVisualizer => []);
