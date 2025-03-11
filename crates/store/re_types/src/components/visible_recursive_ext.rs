use re_types_core::datatypes::Bool;

use super::VisibleRecursive;

impl Default for VisibleRecursive {
    #[inline]
    fn default() -> Self {
        Self(Bool(true))
    }
}
