use crate::{
    hash::Hash128,
    path::{Index, IndexPath, ObjPathBuilder, ObjPathComp, ObjTypePath, TypePathComp},
};

/// `camera / "left" / points / #42`
#[derive(Clone, Debug, Eq)]
pub struct ObjPath {
    /// precomputed hash
    hash: Hash128,

    /// `camera / * / points / *`
    obj_type_path: ObjTypePath,

    /// "left" / #42
    index_path: IndexPath,
}

impl ObjPath {
    pub fn new(obj_type_path: ObjTypePath, index_path: IndexPath) -> Self {
        // TODO: sanity check
        let hash = Hash128::hash((&obj_type_path, &index_path));
        Self {
            obj_type_path,
            index_path,
            hash,
        }
    }

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

    /// Precomputed 64-bit hash.
    #[inline]
    pub fn hash64(&self) -> u64 {
        self.hash.hash64()
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
}

impl From<&ObjPathBuilder> for ObjPath {
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
        ObjPath::new(ObjTypePath::new(obj_type_path), IndexPath::new(index_path))
    }
}

// ----------------------------------------------------------------------------

#[cfg(feature = "serde")]
impl serde::Serialize for ObjPath {
    #[inline]
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        (&self.obj_type_path, &self.index_path).serialize(serializer)
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for ObjPath {
    #[inline]
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let (obj_type_path, index_path) = <(ObjTypePath, IndexPath)>::deserialize(deserializer)?;
        Ok(Self::new(obj_type_path, index_path))
    }
}

// ----------------------------------------------------------------------------

impl std::cmp::PartialEq for ObjPath {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.hash == other.hash // much faster, and low chance of collision
    }
}

impl std::hash::Hash for ObjPath {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.hash.hash(state);
    }
}

impl nohash_hasher::IsEnabled for ObjPath {}

impl From<ObjPathBuilder> for ObjPath {
    fn from(obj_path: ObjPathBuilder) -> Self {
        let mut obj_type_path = Vec::default();
        let mut index_path = IndexPath::default();
        for comp in obj_path {
            match comp {
                ObjPathComp::String(name) => {
                    obj_type_path.push(TypePathComp::String(name));
                }
                ObjPathComp::Index(Index::Placeholder) => {
                    obj_type_path.push(TypePathComp::Index);
                }
                ObjPathComp::Index(index) => {
                    obj_type_path.push(TypePathComp::Index);
                    index_path.push(index);
                }
            }
        }
        ObjPath::new(ObjTypePath::new(obj_type_path), index_path)
    }
}

// ----------------------------------------------------------------------------

impl std::cmp::Ord for ObjPath {
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

impl std::cmp::PartialOrd for ObjPath {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

// ----------------------------------------------------------------------------

#[derive(PartialEq, Eq, PartialOrd, Ord)]
pub enum ObjPathCompRef<'a> {
    /// Struct member. Each member can have a different type.
    String(&'a rr_string_interner::InternedString),

    /// Array/table/map member. Each member must be of the same type (homogenous).
    Index(&'a Index),

    IndexPlaceholder,
}

impl<'a> std::fmt::Display for ObjPathCompRef<'a> {
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
    type Item = ObjPathCompRef<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.obj_type_path.next()? {
            TypePathComp::String(name) => Some(ObjPathCompRef::String(name)),
            TypePathComp::Index => match self.index_path.next() {
                Some(index) => Some(ObjPathCompRef::Index(index)),
                None => Some(ObjPathCompRef::IndexPlaceholder),
            },
        }
    }
}

// ----------------------------------------------------------------------------

impl std::fmt::Display for ObjPath {
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
