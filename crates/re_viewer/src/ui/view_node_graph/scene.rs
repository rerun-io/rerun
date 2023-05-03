use crate::{ui::SceneQuery, ViewerContext};
use re_data_store::EntityPath;
// ---

#[derive(Debug, Clone)]
pub struct NodeGraphEntry {
    pub entity_path: EntityPath,

    /// `None` for timeless data.
    pub time: Option<i64>,

    pub color: Option<[u8; 4]>,

    pub level: Option<String>,

    pub body: String,
}

/// A NodeGraph scene, with everything needed to render it.
#[derive(Default)]
pub struct SceneNodeGraph {
    pub NodeGraph_entries: Vec<NodeGraphEntry>,
}

impl SceneNodeGraph {
    /// Loads all NodeGraph components into the scene according to the given query.
    pub(crate) fn load(&mut self, ctx: &ViewerContext<'_>, query: &SceneQuery<'_>) {
        crate::profile_function!();

        let store = &ctx.log_db.entity_db.data_store;

        for entity_path in query.entity_paths {}
    }
}
