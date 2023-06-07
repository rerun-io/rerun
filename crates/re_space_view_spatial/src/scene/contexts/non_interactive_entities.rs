use nohash_hasher::IntSet;
use re_log_types::EntityPathHash;
use re_viewer_context::SceneContextPart;

/// List of all non-interactive entities for lookup during picking evaluation.
///
/// TODO(wumpf/jleibs): This is a temporary solution until the picking code can query propagated blueprint properties directly.
#[derive(Default)]
pub struct NonInteractiveEntities(pub IntSet<EntityPathHash>);

impl SceneContextPart for NonInteractiveEntities {
    fn archetypes(&self) -> Vec<re_viewer_context::ArchetypeDefinition> {
        Vec::new()
    }

    fn populate(
        &mut self,
        _ctx: &mut re_viewer_context::ViewerContext<'_>,
        query: &re_viewer_context::SceneQuery<'_>,
        _space_view_state: &dyn re_viewer_context::SpaceViewState,
    ) {
        re_tracing::profile_function!();

        self.0 = query
            .entity_props_map
            .iter()
            .filter_map(|(entity_path, props)| {
                if props.interactive {
                    None
                } else {
                    Some(entity_path.hash())
                }
            })
            .collect();
    }
}
