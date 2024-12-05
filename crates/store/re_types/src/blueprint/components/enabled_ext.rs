use re_types_core::datatypes::Bool;

use super::Enabled;

impl Default for Enabled {
    #[inline]
    fn default() -> Self {
        Self(Bool(false))
    }
}

impl From<Enabled> for bool {
    #[inline]
    fn from(v: Enabled) -> Self {
        v.0.into()
    }
}
