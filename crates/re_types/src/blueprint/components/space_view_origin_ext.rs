use re_types_core::datatypes::EntityPath;

use super::SpaceViewOrigin;

impl Default for SpaceViewOrigin {
    fn default() -> Self {
        Self(EntityPath("/*".to_owned().into()))
    }
}
