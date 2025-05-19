use re_types::{Archetype as _, ComponentDescriptorSet, archetypes::AnnotationContext};
use re_viewer_context::{
    AnnotationMap, IdentifiedViewSystem, ViewContextSystem, ViewSystemIdentifier,
};

#[derive(Default)]
pub struct AnnotationSceneContext(pub AnnotationMap);

impl IdentifiedViewSystem for AnnotationSceneContext {
    fn identifier() -> ViewSystemIdentifier {
        "AnnotationSceneContext".into()
    }
}

impl ViewContextSystem for AnnotationSceneContext {
    fn compatible_component_sets(&self) -> Vec<ComponentDescriptorSet> {
        vec![
            AnnotationContext::required_components()
                .iter()
                .cloned()
                .collect(),
        ]
    }

    fn execute(
        &mut self,
        ctx: &re_viewer_context::ViewContext<'_>,
        query: &re_viewer_context::ViewQuery<'_>,
    ) {
        re_tracing::profile_function!();
        // We create a list of *all* entities here, do not only iterate over those with annotation context.
        // TODO(andreas): But knowing ahead of time where we have annotation contexts could be used for optimization.
        self.0.load(
            ctx.viewer_ctx,
            &query.latest_at_query(),
            query.iter_all_entities(),
        );
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
