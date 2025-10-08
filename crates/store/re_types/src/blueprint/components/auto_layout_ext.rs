use crate::datatypes::Bool;

use super::AutoLayout;

impl Default for AutoLayout {
    #[inline]
    fn default() -> Self {
        AutoLayout(Bool(true))
    }
}
