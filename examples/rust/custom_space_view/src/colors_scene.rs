use re_viewer::external::{
    egui, re_components,
    re_data_store::InstancePathHash,
    re_log_types::Component as _,
    re_query::query_entity_with_primary,
    re_renderer,
    re_viewer_context::{
        ArchetypeDefinition, ScenePart, SceneQuery, SpaceViewClass, SpaceViewHighlights,
        ViewerContext,
    },
};

use crate::color_coordinates_space_view::ColorCoordinatesSpaceView;

/// The scene for the [`ColorCoordinatesSpaceView`].
///
/// This is a collection of all information needed to display a single frame for this Space View.
/// The data is queried from the data store here and processed to consumption by the Space View's ui method.
#[derive(Default)]
pub struct ColorsScene {
    pub colors: Vec<(InstancePathHash, egui::Color32)>,
}

// [`SceneColorCoordinates`] is itself its only scene part.
impl ScenePart<ColorCoordinatesSpaceView> for ColorsScene {
    /// The archetype this scene part is querying from the store.
    ///
    /// TODO(wumpf): In future versions there will be a hard restriction that limits the queries within the `populate` method,
    ///              to this here define archetype.
    fn archetype(&self) -> ArchetypeDefinition {
        ArchetypeDefinition::new(re_components::ColorRGBA::name())
    }

    /// Populates the scene part with data from the store.
    fn populate(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        query: &SceneQuery<'_>,
        _space_view_state: &<ColorCoordinatesSpaceView as SpaceViewClass>::State,
        _scene_context: &<ColorCoordinatesSpaceView as SpaceViewClass>::Context,
        _highlights: &SpaceViewHighlights,
    ) -> Vec<re_renderer::QueueableDrawData> {
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
                    self.colors.extend(
                        ent_view
                            .iter_instance_keys()
                            .zip(primary_iterator)
                            .filter_map(|(instance_key, color)| {
                                color.map(|color| {
                                    let [r, g, b, _] = color.to_array();
                                    (
                                        InstancePathHash::instance(ent_path, instance_key),
                                        egui::Color32::from_rgb(r, g, b),
                                    )
                                })
                            }),
                    );
                }
            }
        }

        // We're not using `re_renderer` here, so return an empty vector.
        // For more advanced use cases, instead of drawing everything with egui,
        // you may return one or more `QueueableDrawData` instances that are handled by `re_renderer`.
        Vec::new()
    }
}
