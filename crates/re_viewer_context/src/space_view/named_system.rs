use std::collections::{BTreeMap, BTreeSet};

use re_log_types::EntityPath;

re_string_interner::declare_new_type!(
    /// Unique name for a system within a given SpaceViewClass.
    ///
    /// Note that this is *not* unique across the entire application.
    #[derive(serde::Deserialize, serde::Serialize)]
    pub struct ViewSystemName;
);

impl ViewSystemName {
    // TODO(andreas): We should not need this. Using 'unknown' is risking heuristics to not get up-to-date information.
    pub fn unknown() -> Self {
        "unknown".into()
    }
}

pub type PerSystemEntities = BTreeMap<ViewSystemName, BTreeSet<EntityPath>>;

pub trait NamedViewSystem {
    fn name() -> ViewSystemName;
}
