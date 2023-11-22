use std::fmt::Display;

use crate::EntityPath;

/// An expression that corresponds to multiple [`EntityPath`]s within a tree.
// TODO(jleibs): Globs and other fanciness.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum EntityPathExpr {
    /// A single [`EntityPath`].
    ///
    /// Written as: "foo/bar"
    Exact(EntityPath),

    /// An [`EntityPath`] and all its recursive children
    ///
    /// Written as: "foo/bar/"
    Recursive(EntityPath),
}

impl EntityPathExpr {
    /// Returns the [`EntityPath`] that this expression corresponds to.
    pub fn entity_path(&self) -> &EntityPath {
        match self {
            Self::Exact(path) | Self::Recursive(path) => path,
        }
    }

    /// Returns the [`EntityPath`] for which this expression should do an exact match
    ///
    /// Returns None if this is not an exact expression.
    pub fn exact_entity_path(&self) -> Option<&EntityPath> {
        match self {
            Self::Exact(path) => Some(path),
            Self::Recursive(_) => None,
        }
    }

    /// Returns the [`EntityPath`] for which this expression should do a recursive search
    ///
    /// Returns None if this is not a recursive expression.
    pub fn recursive_entity_path(&self) -> Option<&EntityPath> {
        match self {
            Self::Exact(_) => None,
            Self::Recursive(path) => Some(path),
        }
    }
}

impl Display for EntityPathExpr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Exact(path) => path.fmt(f),
            Self::Recursive(path) => write!(f, "{path}/"),
        }
    }
}

impl From<&str> for EntityPathExpr {
    #[inline]
    fn from(path: &str) -> Self {
        if let Some(path) = path.strip_suffix('/') {
            Self::Recursive(EntityPath::from(path))
        } else {
            Self::Exact(EntityPath::from(path))
        }
    }
}
