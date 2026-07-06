use std::collections::{BTreeMap, BTreeSet};

use re_log_types::EntityPath;

re_string_interner::declare_new_type_nonempty!(
    /// Unique name for a system within a given [`crate::ViewClass`].
    ///
    /// Note that this is *not* unique across the entire application.
    pub struct ViewSystemIdentifier;
);

impl Default for ViewSystemIdentifier {
    fn default() -> Self {
        re_string_interner::intern_static_nonempty!(ViewSystemIdentifier, "unknown")
    }
}

// TODO(andreas): This should likely be `PerVisualizer<SelectedEntities>` instead.
//                Implying the missing concept of `SelectedEntities` which is a subset of `VisualizableEntities`
//                as selected by the query.
pub type PerSystemEntities = BTreeMap<ViewSystemIdentifier, BTreeSet<EntityPath>>;

/// Trait for naming/identifying [`crate::VisualizerSystem`]s & [`crate::ViewContextSystem`]s.
///
/// Required to be implemented for registration.
pub trait IdentifiedViewSystem {
    /// Unique name for a system within a given [`crate::ViewClass`].
    ///
    /// Note that this is *not* unique across the entire application.
    fn identifier() -> ViewSystemIdentifier;
}
