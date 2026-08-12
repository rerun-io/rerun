use re_types_core::encodings::Bool;

use super::Visible;

impl Default for Visible {
    #[inline]
    fn default() -> Self {
        Self(Bool(true))
    }
}
