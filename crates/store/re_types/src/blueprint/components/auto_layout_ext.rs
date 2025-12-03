use super::AutoLayout;
use crate::datatypes::Bool;

impl Default for AutoLayout {
    #[inline]
    fn default() -> Self {
        Self(Bool(true))
    }
}
