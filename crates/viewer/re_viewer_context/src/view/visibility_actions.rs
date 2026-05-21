use re_entity_db::EntityPath;

use crate::ViewerContext;

/// Apply the given `visible` value to every view whose query result contains
/// `entity_path`, using each peer view's own [`crate::DataResult::save_visible`]
/// to preserve per-view smart-clear semantics.
///
/// Views that do not contain `entity_path` are skipped — writing an override
/// there would just be blueprint noise (the view has no UI to toggle it).
pub fn set_entity_visibility_in_all_views(
    _ctx: &ViewerContext<'_>,
    _entity_path: &EntityPath,
    _visible: bool,
) {
    // Implemented in a follow-up step after a failing test pins the behavior.
}
