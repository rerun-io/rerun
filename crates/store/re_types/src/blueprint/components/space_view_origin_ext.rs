use re_types_core::datatypes::EntityPath;

use super::SpaceViewOrigin;

impl Default for SpaceViewOrigin {
    #[inline]
    fn default() -> Self {
        Self(EntityPath("/*".to_owned().into()))
    }
}
