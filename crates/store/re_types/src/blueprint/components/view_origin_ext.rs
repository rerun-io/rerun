use re_types_core::datatypes::EntityPath;

use super::ViewOrigin;

impl Default for ViewOrigin {
    #[inline]
    fn default() -> Self {
        Self(EntityPath("/*".to_owned().into()))
    }
}
