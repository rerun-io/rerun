use crate::{
    path::{IndexPath, ObjPathComp, ObjTypePath, ObjTypePathComp},
    Index,
};

/// `camera / "left" / points / #42`
///
/// Wrapped by [`crate::ObjPath`] together with a hash.
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

    /// Number of components
    #[inline]
    pub fn len(&self) -> usize {
        self.obj_type_path.len()
    }

    #[inline]
    pub fn obj_type_path(&self) -> &ObjTypePath {
        &self.obj_type_path
    }

    #[inline]
    pub fn index_path(&self) -> &IndexPath {
        &self.index_path
    }

    pub fn to_components(&self) -> Vec<ObjPathComp> {
        self.iter().map(|comp_ref| comp_ref.to_owned()).collect()
    }

    #[inline]
    pub fn into_type_path_and_index_path(self) -> (ObjTypePath, IndexPath) {
        (self.obj_type_path, self.index_path)
    }

    #[inline]
    pub fn to_type_path_and_index_path(&self) -> (ObjTypePath, IndexPath) {
        self.clone().into_type_path_and_index_path()
    }

    /// Return [`None`] if root.
    #[must_use]
    pub fn parent(&self) -> Option<Self> {
        let mut obj_type_path = self.obj_type_path.as_slice().to_vec();
        let mut index_path = self.index_path.as_slice().to_vec();

        if matches!(obj_type_path.pop()?, ObjTypePathComp::Index) {
            index_path.pop();
        }

        Some(Self::new(
            ObjTypePath::new(obj_type_path),
            IndexPath::new(index_path),
        ))
    }
}

// ----------------------------------------------------------------------------

impl<'a, It> From<It> for ObjPathImpl
where
    It: Iterator<Item = &'a ObjPathComp>,
{
    fn from(path: It) -> Self {
        let mut obj_type_path = vec![];
        let mut index_path = vec![];
        for comp in path {
            match comp {
                ObjPathComp::Name(name) => {
                    obj_type_path.push(ObjTypePathComp::Name(*name));
                }
                ObjPathComp::Index(index) => {
                    obj_type_path.push(ObjTypePathComp::Index);
                    index_path.push(index.clone());
                }
            }
        }
        ObjPathImpl::new(ObjTypePath::new(obj_type_path), IndexPath::new(index_path))
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

/// A reference to a [`ObjPathComp`].
#[derive(PartialEq, Eq, PartialOrd, Ord)]
pub enum ObjPathCompRef<'a> {
    /// Struct member. Each member can have a different type.
    Name(&'a re_string_interner::InternedString),

    /// Array/table/map member. Each member must be of the same type (homogenous).
    Index(&'a Index),
}

impl<'a> ObjPathCompRef<'a> {
    fn to_owned(&self) -> ObjPathComp {
        match self {
            Self::Name(name) => ObjPathComp::Name(**name),
            Self::Index(index) => ObjPathComp::Index((*index).clone()),
        }
    }
}

impl<'a> std::fmt::Display for ObjPathCompRef<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Name(name) => name.fmt(f),
            Self::Index(index) => index.fmt(f),
        }
    }
}

pub struct Iter<'a> {
    obj_type_path: crate::path::obj_type_path::Iter<'a>,
    index_path: crate::path::index_path::Iter<'a>,
}

impl<'a> Iterator for Iter<'a> {
    type Item = ObjPathCompRef<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.obj_type_path.next()? {
            ObjTypePathComp::Name(name) => Some(ObjPathCompRef::Name(name)),
            ObjTypePathComp::Index => Some(ObjPathCompRef::Index(self.index_path.next()?)),
        }
    }
}

// ----------------------------------------------------------------------------

impl std::fmt::Display for ObjPathImpl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use std::fmt::Write as _;

        let mut iter = self.iter();
        if let Some(first_comp) = iter.next() {
            // no leading nor trailing slash
            first_comp.fmt(f)?;
            for comp in iter {
                f.write_char('/')?;
                comp.fmt(f)?;
            }
            Ok(())
        } else {
            f.write_char('/') // root
        }
    }
}
