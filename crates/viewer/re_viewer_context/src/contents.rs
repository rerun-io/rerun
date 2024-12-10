use std::convert::{TryFrom, TryInto};

use egui_tiles::TileId;

use re_log_types::EntityPath;

use crate::item::Item;
use crate::{BlueprintId, BlueprintIdRegistry, ContainerId, ViewId};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Contents {
    Container(ContainerId),
    View(ViewId),
}

impl Contents {
    pub fn try_from(path: &EntityPath) -> Option<Self> {
        if path.starts_with(ViewId::registry()) {
            Some(Self::View(ViewId::from_entity_path(path)))
        } else if path.starts_with(ContainerId::registry()) {
            Some(Self::Container(ContainerId::from_entity_path(path)))
        } else {
            None
        }
    }

    #[inline]
    pub fn as_entity_path(&self) -> EntityPath {
        match self {
            Self::Container(id) => id.as_entity_path(),
            Self::View(id) => id.as_entity_path(),
        }
    }

    #[inline]
    pub fn as_tile_id(&self) -> TileId {
        match self {
            Self::Container(id) => blueprint_id_to_tile_id(id),
            Self::View(id) => blueprint_id_to_tile_id(id),
        }
    }

    #[inline]
    pub fn as_item(&self) -> Item {
        match self {
            Self::Container(container_id) => Item::Container(*container_id),
            Self::View(view_id) => Item::View(*view_id),
        }
    }

    #[inline]
    pub fn as_container_id(&self) -> Option<ContainerId> {
        match self {
            Self::Container(id) => Some(*id),
            Self::View(_) => None,
        }
    }

    #[inline]
    pub fn as_view_id(&self) -> Option<ViewId> {
        match self {
            Self::View(id) => Some(*id),
            Self::Container(_) => None,
        }
    }
}

impl TryFrom<Item> for Contents {
    type Error = ();

    fn try_from(item: Item) -> Result<Self, Self::Error> {
        (&item).try_into()
    }
}

impl TryFrom<&Item> for Contents {
    type Error = ();

    fn try_from(item: &Item) -> Result<Self, Self::Error> {
        match item {
            Item::Container(id) => Ok(Self::Container(*id)),
            Item::View(id) => Ok(Self::View(*id)),
            _ => Err(()),
        }
    }
}

impl From<ViewId> for Contents {
    #[inline]
    fn from(id: ViewId) -> Self {
        Self::View(id)
    }
}

impl From<ContainerId> for Contents {
    #[inline]
    fn from(id: ContainerId) -> Self {
        Self::Container(id)
    }
}

/// The name of a [`Contents`].
#[derive(Clone, Debug)]
pub enum ContentsName {
    /// This [`Contents`] has been given a name by the user.
    Named(String),

    /// This [`Contents`] is unnamed and should be displayed with this placeholder name.
    Placeholder(String),
}

impl std::fmt::Display for ContentsName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_ref())
    }
}

impl AsRef<str> for ContentsName {
    #[inline]
    fn as_ref(&self) -> &str {
        match self {
            Self::Named(name) | Self::Placeholder(name) => name,
        }
    }
}

#[inline]
pub fn blueprint_id_to_tile_id<T: BlueprintIdRegistry>(id: &BlueprintId<T>) -> TileId {
    TileId::from_u64(id.hash())
}
