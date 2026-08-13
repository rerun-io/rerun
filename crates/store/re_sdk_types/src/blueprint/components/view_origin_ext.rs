use re_types_core::encodings::EntityPath;

use super::ViewOrigin;

impl Default for ViewOrigin {
    #[inline]
    fn default() -> Self {
        Self(EntityPath("/*".to_owned().into()))
    }
}
