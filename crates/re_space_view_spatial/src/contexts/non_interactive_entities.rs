use nohash_hasher::IntSet;
use re_log_types::EntityPathHash;
use re_types::ComponentName;
use re_viewer_context::{NamedViewSystem, ViewContextSystem};

/// List of all non-interactive entities for lookup during picking evaluation.
///
/// TODO(wumpf/jleibs): This is a temporary solution until the picking code can query propagated blueprint properties directly.
#[derive(Default)]
pub struct NonInteractiveEntities(pub IntSet<EntityPathHash>);

impl NamedViewSystem for NonInteractiveEntities {
    fn name() -> re_viewer_context::ViewSystemName {
        "NonInteractiveEntities".into()
    }
}

impl ViewContextSystem for NonInteractiveEntities {
    fn all_required_components(&self) -> Vec<IntSet<ComponentName>> {
        Vec::new()
    }

    fn execute(
        &mut self,
        _ctx: &mut re_viewer_context::ViewerContext<'_>,
        query: &re_viewer_context::ViewQuery<'_>,
    ) {
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

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
