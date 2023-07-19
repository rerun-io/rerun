use re_viewer::external::{
    egui, re_components,
    re_log_types::{EntityPath, InstanceKey},
    re_query::query_entity_with_primary,
    re_renderer,
    re_types::Loggable as _,
    re_viewer_context::{
        ArchetypeDefinition, SpaceViewSystemExecutionError, ViewContextCollection, ViewPartSystem,
        ViewQuery, ViewerContext,
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

impl ViewPartSystem for InstanceColorSystem {
    /// The archetype this scene part is querying from the store.
    ///
    /// TODO(wumpf): In future versions there will be a hard restriction that limits the queries
    ///              within the `populate` method to this archetype.
    fn archetype(&self) -> ArchetypeDefinition {
        ArchetypeDefinition::new(re_components::ColorRGBA::name())
    }

    /// Populates the scene part with data from the store.
    fn execute(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        query: &ViewQuery<'_>,
        _view_ctx: &ViewContextCollection,
    ) -> Result<Vec<re_renderer::QueueableDrawData>, SpaceViewSystemExecutionError> {
        // For each entity in the space view...
        for (ent_path, props) in query.iter_entities() {
            if !props.visible {
                continue;
            }

            // ...gather all colors and their instance ids.
            if let Ok(ent_view) = query_entity_with_primary::<re_components::ColorRGBA>(
                &ctx.store_db.entity_db.data_store,
                &ctx.current_query(),
                ent_path,
                &[re_components::ColorRGBA::name()],
            ) {
                if let Ok(primary_iterator) = ent_view.iter_primary() {
                    self.colors.push((
                        ent_path.clone(),
                        ent_view
                            .iter_instance_keys()
                            .zip(primary_iterator)
                            .filter_map(|(instance_key, color)| {
                                color.map(|color| {
                                    let [r, g, b, _] = color.to_array();
                                    ColorWithInstanceKey {
                                        color: egui::Color32::from_rgb(r, g, b),
                                        instance_key,
                                    }
                                })
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
