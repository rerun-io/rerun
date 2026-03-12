use crate::{Item, ItemContext};

/// One-shot focus payload.
///
/// Unlike selection, focus is cleared every frame. The optional context lets a receiver
/// distinguish between “focus the whole entity” and “focus the exact 3D point that was
/// double-clicked”.
#[derive(Clone, Debug, PartialEq)]
pub struct FocusTarget {
    pub item: Item,
    pub context: Option<ItemContext>,
}

impl From<Item> for FocusTarget {
    #[inline]
    fn from(item: Item) -> Self {
        Self {
            item,
            context: None,
        }
    }
}
