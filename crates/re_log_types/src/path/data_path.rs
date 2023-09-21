use re_types::{components::InstanceKey, ComponentName};

use crate::EntityPath;

/// A general path to some data.
///
/// This always starts with an [`EntityPath`], followed
/// by an optional [`InstanceKey`], followed by an optional [`ComponentName`].
///
/// For instance:
///
/// * `points`
/// * `points.color`
/// * `points[#42]`
/// * `points[#42].color`
pub struct DataPath {
    pub entity_path: EntityPath,

    pub instance_key: Option<InstanceKey>,

    pub component_name: Option<ComponentName>,
}
