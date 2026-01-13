use re_log_types::{EntityPath, EntityPathPart};
use re_types_core::Archetype as _;

use crate::blueprint::archetypes::ViewContents;

impl ViewContents {
    /// The prefix for entity override paths.
    ///
    /// Has to be kept in sync with similar occurrences in other SDK languages.
    const OVERRIDES_PREFIX: &'static str = "overrides";

    /// Visualizers prefix.
    ///
    /// At this prefix we store entity global information.
    /// After that come visualizer instruction ids in the path hierarchy.
    const VISUALIZERS_PREFIX: &'static str = "visualizers";

    /// Entity path for a given view id in the store.
    fn blueprint_entity_path_for_view_id(view_id: uuid::Uuid) -> EntityPath {
        EntityPath::new(vec![
            EntityPathPart::new("view"),
            EntityPathPart::new(view_id.to_string()),
            EntityPathPart::new(Self::name().short_name()),
        ])
    }

    /// Base override path for a given entity in a given view.
    ///
    /// Visualizer instruction types and overrides are stored under `<this path>/<visualizer id>`.
    pub fn blueprint_base_visualizer_path_for_entity(
        view_id: uuid::Uuid,
        entity_path: &EntityPath,
    ) -> EntityPath {
        Self::blueprint_entity_path_for_view_id(view_id)
            .join(&EntityPath::from_single_string(Self::OVERRIDES_PREFIX))
            .join(entity_path)
            .join(&EntityPath::from_single_string(Self::VISUALIZERS_PREFIX))
    }
}
