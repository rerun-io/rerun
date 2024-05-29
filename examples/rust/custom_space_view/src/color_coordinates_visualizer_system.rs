use re_viewer::external::{
    egui,
    re_log_types::{EntityPath, Instance},
    re_query, re_renderer,
    re_types::{self, components::Color, ComponentName, Loggable as _},
    re_viewer_context::{
        IdentifiedViewSystem, SpaceViewSystemExecutionError, ViewContextCollection, ViewQuery,
        ViewSystemIdentifier, ViewerContext, VisualizerQueryInfo, VisualizerSystem,
    },
};

/// Our space view consist of single part which holds a list of egui colors for each entity path.
#[derive(Default)]
pub struct InstanceColorSystem {
    pub colors: Vec<(EntityPath, Vec<ColorWithInstance>)>,
}

pub struct ColorWithInstance {
    pub color: egui::Color32,
    pub instance: Instance,
}

struct ColorArchetype;

impl re_types::Archetype for ColorArchetype {
    type Indicator = re_types::GenericIndicatorComponent<Self>;

    fn name() -> re_types::ArchetypeName {
        "InstanceColor".into()
    }

    fn display_name() -> &'static str {
        "Instance Color"
    }

    fn required_components() -> ::std::borrow::Cow<'static, [ComponentName]> {
        vec![re_types::components::Color::name()].into()
    }
}

impl re_query::ToArchetype<ColorArchetype> for re_query::LatestAtResults {
    #[inline]
    fn to_archetype(
        &self,
        _resolver: &re_query::PromiseResolver,
    ) -> re_query::PromiseResult<re_query::Result<ColorArchetype>> {
        re_query::PromiseResult::Ready(Ok(ColorArchetype))
    }
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

    /// Populates the scene part with data from the store.
    fn execute(
        &mut self,
        ctx: &ViewerContext<'_>,
        query: &ViewQuery<'_>,
        _view_ctx: &ViewContextCollection,
    ) -> Result<Vec<re_renderer::QueueableDrawData>, SpaceViewSystemExecutionError> {
        // For each entity in the space view that should be displayed with the `InstanceColorSystem`…
        for data_result in query.iter_visible_data_results(ctx, Self::identifier()) {
            // …gather all colors and their instance ids.

            let results = ctx.recording().query_caches().latest_at(
                ctx.recording_store(),
                &ctx.current_query(),
                &data_result.entity_path,
                [Color::name()],
            );

            let Some(colors) = results.get(Color::name()).and_then(|results| {
                results
                    .to_dense::<Color>(ctx.recording().resolver())
                    .flatten()
                    .ok()
            }) else {
                continue;
            };

            self.colors.push((
                data_result.entity_path.clone(),
                (0..)
                    .zip(colors)
                    .map(|(instance, color)| {
                        let [r, g, b, _] = color.to_array();
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
}
