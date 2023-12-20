use nohash_hasher::IntSet;
use re_log_types::EntityPathHash;
use re_types::ComponentNameSet;
use re_viewer_context::{IdentifiedViewSystem, ViewContextSystem};

/// List of all non-interactive entities for lookup during picking evaluation.
///
/// TODO(wumpf, jleibs): This is a temporary solution until the picking code can query propagated blueprint properties directly.
#[derive(Default)]
pub struct NonInteractiveEntities(pub IntSet<EntityPathHash>);

impl IdentifiedViewSystem for NonInteractiveEntities {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "NonInteractiveEntities".into()
    }
}

impl ViewContextSystem for NonInteractiveEntities {
    fn compatible_component_sets(&self) -> Vec<ComponentNameSet> {
        Vec::new()
    }

    fn execute(
        &mut self,
        _ctx: &re_viewer_context::ViewerContext<'_>,
        query: &re_viewer_context::ViewQuery<'_>,
    ) {
        self.0 = query
            .iter_all_data_results()
            .filter_map(|data_result| {
                if data_result.accumulated_properties().interactive {
                    None
                } else {
                    Some(data_result.entity_path.hash())
                }
            })
            .collect();
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
