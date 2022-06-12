use crate::{
    hash::Hash128,
    path::{ObjPathBuilder, ObjPathComp, TypePathComp},
};

/// The shared type path for all objects at a path with different indices
///
/// `camera / * / points / *`
#[derive(Clone, Debug, Eq)]
pub struct ObjTypePath {
    components: Vec<TypePathComp>,
    hash: Hash128,
}

impl ObjTypePath {
    #[inline]
    pub fn new(components: Vec<TypePathComp>) -> Self {
        let hash = Hash128::hash(&components);
        Self { components, hash }
    }

    #[inline]
    pub fn as_slice(&self) -> &[TypePathComp] {
        self.components.as_slice()
    }

    #[inline]
    pub fn is_root(&self) -> bool {
        self.components.is_empty()
    }

    #[inline]
    pub fn iter(&self) -> Iter<'_> {
        self.components.iter()
    }

    pub fn push(&mut self, comp: TypePathComp) {
        self.components.push(comp);
        self.hash = Hash128::hash(&self.components);
    }
}

// ----------------------------------------------------------------------------

#[cfg(feature = "serde")]
impl serde::Serialize for ObjTypePath {
    #[inline]
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.as_slice().serialize(serializer)
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for ObjTypePath {
    #[inline]
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        <Vec<TypePathComp>>::deserialize(deserializer).map(ObjTypePath::new)
    }
}

// ----------------------------------------------------------------------------

impl std::cmp::PartialEq for ObjTypePath {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.hash == other.hash // much faster, and low chance of collision
    }
}

impl std::hash::Hash for ObjTypePath {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.hash.hash(state);
    }
}

impl nohash_hasher::IsEnabled for ObjTypePath {}

impl std::cmp::PartialOrd for ObjTypePath {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.components.partial_cmp(&other.components)
    }
}

impl std::cmp::Ord for ObjTypePath {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.components.cmp(&other.components)
    }
}

// ----------------------------------------------------------------------------

pub type Iter<'a> = std::slice::Iter<'a, TypePathComp>;

impl From<ObjPathBuilder> for ObjTypePath {
    fn from(obj_path_builder: ObjPathBuilder) -> Self {
        ObjTypePath::new(
            obj_path_builder
                .iter()
                .map(ObjPathComp::to_type_path_comp)
                .collect(),
        )
    }
}

impl<'a> IntoIterator for &'a ObjTypePath {
    type Item = &'a TypePathComp;
    type IntoIter = std::slice::Iter<'a, TypePathComp>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.components.iter()
    }
}

impl IntoIterator for ObjTypePath {
    type Item = TypePathComp;
    type IntoIter = <Vec<TypePathComp> as IntoIterator>::IntoIter;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.components.into_iter()
    }
}

impl std::fmt::Display for ObjTypePath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use std::fmt::Write as _;

        f.write_char('/')?;
        for (i, comp) in self.components.iter().enumerate() {
            comp.fmt(f)?;
            if i + 1 != self.components.len() {
                f.write_char('/')?;
            }
        }
        Ok(())
    }
}

impl From<&str> for ObjTypePath {
    #[inline]
    fn from(component: &str) -> Self {
        Self::new(vec![TypePathComp::String(component.into())])
    }
}

impl From<TypePathComp> for ObjTypePath {
    #[inline]
    fn from(component: TypePathComp) -> Self {
        Self::new(vec![component])
    }
}

impl std::ops::Div for TypePathComp {
    type Output = ObjTypePath;

    #[inline]
    fn div(self, rhs: TypePathComp) -> Self::Output {
        ObjTypePath::new(vec![self, rhs])
    }
}

impl std::ops::Div<TypePathComp> for ObjTypePath {
    type Output = ObjTypePath;

    #[inline]
    fn div(mut self, rhs: TypePathComp) -> Self::Output {
        self.push(rhs);
        self
    }
}

impl std::ops::Div<&str> for ObjTypePath {
    type Output = ObjTypePath;

    #[inline]
    fn div(mut self, rhs: &str) -> Self::Output {
        self.push(TypePathComp::String(rhs.into()));
        self
    }
}

impl std::ops::Div<&str> for &ObjTypePath {
    type Output = ObjTypePath;

    #[inline]
    fn div(self, rhs: &str) -> Self::Output {
        self.clone() / rhs
    }
}
