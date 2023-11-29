use std::collections::{BTreeMap, BTreeSet};

use re_log_types::EntityPath;

re_string_interner::declare_new_type!(
    /// Unique name for a system within a given [`crate::SpaceViewClass`].
    ///
    /// Note that this is *not* unique across the entire application.
    #[derive(serde::Deserialize, serde::Serialize)]
    pub struct ViewSystemName;
);

impl Default for ViewSystemName {
    fn default() -> Self {
        "unknown".into()
    }
}

pub type PerSystemEntities = BTreeMap<ViewSystemName, BTreeSet<EntityPath>>;

/// Trait for naming/identifying [`crate::ViewPartSystem`]s & [`crate::ViewContextSystem`]s.
///
/// Required to be implemented for registration.
pub trait NamedViewSystem {
    /// Unique name for this system.
    const NAME: &'static str;

    /// Unique name for a system within a given [`crate::SpaceViewClass`].
    ///
    /// Note that this is *not* unique across the entire application.
    fn name() -> ViewSystemName {
        Self::NAME.into()
    }
}
