use crate::{
    path::{IndexPath, ObjPathBuilder, ObjPathComp, ObjTypePath, TypePathComp},
    Index,
};

/// `camera / "left" / points / #42`
///
/// Wrapped by [`ObjPath`] together with a hash.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub(crate) struct ObjPathImpl {
    /// `camera / * / points / *`
    obj_type_path: ObjTypePath,

    /// "left" / #42
    index_path: IndexPath,
}

impl ObjPathImpl {
    #[inline]
    pub fn root() -> Self {
        Self::new(ObjTypePath::root(), IndexPath::default())
    }

    pub fn new(obj_type_path: ObjTypePath, index_path: IndexPath) -> Self {
        assert_eq!(
            obj_type_path.num_indices(),
            index_path.len(),
            "Bad object path: mismatched indices. Type path: {}, index path: {:?}",
            obj_type_path,
            index_path
        );

        Self {
            obj_type_path,
            index_path,
        }
    }

    #[inline]
    pub fn iter(&self) -> Iter<'_> {
        Iter {
            obj_type_path: self.obj_type_path.iter(),
            index_path: self.index_path.iter(),
        }
    }

    #[inline]
    pub fn is_root(&self) -> bool {
        self.obj_type_path.is_root()
    }

    #[inline]
    pub fn obj_type_path(&self) -> &ObjTypePath {
        &self.obj_type_path
    }

    #[inline]
    pub fn index_path(&self) -> &IndexPath {
        &self.index_path
    }

    #[inline]
    pub fn into_type_path_and_index_path(self) -> (ObjTypePath, IndexPath) {
        (self.obj_type_path, self.index_path)
    }

    #[must_use]
    pub fn parent(&self) -> Self {
        let mut obj_type_path = self.obj_type_path.as_slice().to_vec();
        let mut index_path = self.index_path.as_slice().to_vec();

        if matches!(obj_type_path.pop(), Some(TypePathComp::Index)) {
            index_path.pop();
        }

        Self::new(ObjTypePath::new(obj_type_path), IndexPath::new(index_path))
    }

    /// Replace last [`Index::Placeholder`] with the given key.
    #[must_use]
    pub fn replace_last_placeholder_with(self, key: crate::IndexKey) -> Self {
        let (type_path, mut index_path) = self.into_type_path_and_index_path();
        index_path.replace_last_placeholder_with(key);
        Self::new(type_path, index_path)
    }
}

impl From<&ObjPathBuilder> for ObjPathImpl {
    #[inline]
    fn from(path: &ObjPathBuilder) -> Self {
        let mut obj_type_path = vec![];
        let mut index_path = vec![];
        for comp in path.iter() {
            match comp {
                ObjPathComp::String(name) => {
                    obj_type_path.push(TypePathComp::String(*name));
                }
                ObjPathComp::Index(index) => {
                    obj_type_path.push(TypePathComp::Index);
                    index_path.push(index.clone());
                }
            }
        }
        ObjPathImpl::new(ObjTypePath::new(obj_type_path), IndexPath::new(index_path))
    }
}

// ----------------------------------------------------------------------------

impl From<ObjPathBuilder> for ObjPathImpl {
    fn from(obj_path: ObjPathBuilder) -> Self {
        let mut obj_type_path = Vec::default();
        let mut index_path = IndexPath::default();
        for comp in obj_path {
            match comp {
                ObjPathComp::String(name) => {
                    obj_type_path.push(TypePathComp::String(name));
                }
                ObjPathComp::Index(index) => {
                    obj_type_path.push(TypePathComp::Index);
                    index_path.push(index);
                }
            }
        }
        ObjPathImpl::new(ObjTypePath::new(obj_type_path), index_path)
    }
}

// ----------------------------------------------------------------------------

impl std::cmp::Ord for ObjPathImpl {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        use std::cmp::Ordering;

        let mut lhs = self.iter();
        let mut rhs = other.iter();

        loop {
            match (lhs.next(), rhs.next()) {
                (None, None) => return Ordering::Equal,
                (None, Some(_)) => return Ordering::Less,
                (Some(_), None) => return Ordering::Greater,
                (Some(lhs), Some(rhs)) => match lhs.cmp(&rhs) {
                    Ordering::Equal => {}
                    ordering => return ordering,
                },
            }
        }
    }
}

impl std::cmp::PartialOrd for ObjPathImpl {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

// ----------------------------------------------------------------------------

#[derive(PartialEq, Eq, PartialOrd, Ord)]
pub enum ObjPathComponentRef<'a> {
    /// Struct member. Each member can have a different type.
    String(&'a rr_string_interner::InternedString),

    /// Array/table/map member. Each member must be of the same type (homogenous).
    Index(&'a Index),

    IndexPlaceholder,
}

impl<'a> std::fmt::Display for ObjPathComponentRef<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::String(name) => name.fmt(f),
            Self::Index(index) => index.fmt(f),
            Self::IndexPlaceholder => '_'.fmt(f),
        }
    }
}

pub struct Iter<'a> {
    obj_type_path: crate::path::obj_type_path::Iter<'a>,
    index_path: crate::path::index_path::Iter<'a>,
}

impl<'a> Iterator for Iter<'a> {
    type Item = ObjPathComponentRef<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.obj_type_path.next()? {
            TypePathComp::String(name) => Some(ObjPathComponentRef::String(name)),
            TypePathComp::Index => match self.index_path.next() {
                Some(index) => Some(ObjPathComponentRef::Index(index)),
                None => Some(ObjPathComponentRef::IndexPlaceholder),
            },
        }
    }
}

// ----------------------------------------------------------------------------

impl std::fmt::Display for ObjPathImpl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use std::fmt::Write as _;

        let mut any = false;
        for comp in self.iter() {
            f.write_char('/')?;
            comp.fmt(f)?;
            any |= true;
        }

        if any {
            Ok(())
        } else {
            f.write_char('/')
        }
    }
}
