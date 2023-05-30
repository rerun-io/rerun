use ahash::HashMap;

use crate::space_view_type::SpaceViewType;

/// Registry of all known space view types.
///
/// Expected to be populated on viewer startup.
#[derive(Default)]
pub struct SpaceViewTypeRegistry(HashMap<std::any::TypeId, Box<dyn SpaceViewType>>);
