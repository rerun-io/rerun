use re_viewer::external::{
    egui,
    re_log_types::EntityPath,
    re_query::query_archetype,
    re_renderer,
    re_types::{self, components::InstanceKey, Archetype, ComponentName, Loggable as _},
    re_viewer_context::{
        ArchetypeDefinition, NamedViewSystem, SpaceViewSystemExecutionError, ViewContextCollection,
        ViewPartSystem, ViewQuery, ViewSystemName, ViewerContext,
    },
};

/// Our space view consist of single part which holds a list of egui colors for each entity path.
#[derive(Default)]
pub struct InstanceColorSystem {
    pub colors: Vec<(EntityPath, Vec<ColorWithInstanceKey>)>,
}

pub struct ColorWithInstanceKey {
    pub color: egui::Color32,
    pub instance_key: InstanceKey,
}

struct ColorArchetype;

// TODO(#2778): The introduction of ArchetypeInfo should make this much much nicer.
impl re_types::Archetype for ColorArchetype {
    fn name() -> re_types::ArchetypeName {
        "InstanceColor".into()
    }

    fn required_components() -> ::std::borrow::Cow<'static, [ComponentName]> {
        vec![re_types::components::Color::name()].into()
    }
}

impl NamedViewSystem for InstanceColorSystem {
    fn name() -> ViewSystemName {
        "InstanceColor".into()
    }
}

impl ViewPartSystem for InstanceColorSystem {
    /// The archetype this scene part is querying from the store.
    ///
    /// TODO(wumpf): In future versions there will be a hard restriction that limits the queries
    ///              within the `populate` method to this archetype.
    fn archetype(&self) -> ArchetypeDefinition {
        ColorArchetype::all_components().try_into().unwrap()
    }

    /// Populates the scene part with data from the store.
    fn execute(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        query: &ViewQuery<'_>,
        _view_ctx: &ViewContextCollection,
    ) -> Result<Vec<re_renderer::QueueableDrawData>, SpaceViewSystemExecutionError> {
        // For each entity in the space view that should be displayed with the the `InstanceColorSystem`...
        for (ent_path, _props) in query.iter_entities_for_system(InstanceColorSystem::name()) {
            // ...gather all colors and their instance ids.
            if let Ok(arch_view) = query_archetype::<ColorArchetype>(
                &ctx.store_db.entity_db.data_store,
                &ctx.current_query(),
                ent_path,
            ) {
                if let Ok(colors) =
                    arch_view.iter_required_component::<re_types::components::Color>()
                {
                    self.colors.push((
                        ent_path.clone(),
                        arch_view
                            .iter_instance_keys()
                            .zip(colors)
                            .map(|(instance_key, color)| {
                                let [r, g, b, _] = color.to_array();
                                ColorWithInstanceKey {
                                    color: egui::Color32::from_rgb(r, g, b),
                                    instance_key,
                                }
                            })
                            .collect(),
                    ));
                }
            }
        }

        // We're not using `re_renderer` here, so return an empty vector.
        // If you want to draw additional primitives here, you can emit re_renderer draw data here directly.
        Ok(Vec::new())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
