use ahash::HashMap;

use crate::space_view_type::SpaceViewType;

/// Registry of all known space view types.
///
/// Populated on viewer startup. Add additive only.
#[derive(Default)]
pub struct SpaceViewTypeRegistry(HashMap<std::any::TypeId, Box<dyn SpaceViewType>>);
