use crate::{hash::Hash64, path::ObjTypePathComp, ObjPathComp};

/// The shared type path for all objects at a path with different indices
///
/// `camera / * / points / *`
#[derive(Clone, Eq)]
pub struct ObjTypePath {
    // 64 bit is enough, because we will have at most a few thousand unique type paths - never a billion.
    hash: Hash64,
    components: Vec<ObjTypePathComp>, // TODO(emilk): box?
}

impl ObjTypePath {
    #[inline]
    pub fn root() -> Self {
        Self::new(vec![])
    }

    pub fn new(components: Vec<ObjTypePathComp>) -> Self {
        let hash = Hash64::hash(&components);
        Self { components, hash }
    }

    #[inline]
    pub fn as_slice(&self) -> &[ObjTypePathComp] {
        self.components.as_slice()
    }

    #[inline]
    pub fn is_root(&self) -> bool {
        self.components.is_empty()
    }

    /// Number of components
    #[inline]
    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        self.components.len()
    }

    #[inline]
    pub fn iter(&self) -> Iter<'_> {
        self.components.iter()
    }

    pub fn push(&mut self, comp: ObjTypePathComp) {
        self.components.push(comp);
        self.hash = Hash64::hash(&self.components);
    }

    pub fn num_indices(&self) -> usize {
        self.components
            .iter()
            .filter(|c| match c {
                ObjTypePathComp::Name(_) => false,
                ObjTypePathComp::Index => true,
            })
            .count()
    }

    pub fn to_components(&self) -> Vec<ObjTypePathComp> {
        self.components.clone()
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
        <Vec<ObjTypePathComp>>::deserialize(deserializer).map(ObjTypePath::new)
    }
}

// ----------------------------------------------------------------------------

impl std::cmp::PartialEq for ObjTypePath {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.hash == other.hash // much faster, and low risk of collision
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

pub type Iter<'a> = std::slice::Iter<'a, ObjTypePathComp>;

impl<'a> IntoIterator for &'a ObjTypePath {
    type Item = &'a ObjTypePathComp;
    type IntoIter = std::slice::Iter<'a, ObjTypePathComp>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.components.iter()
    }
}

impl IntoIterator for ObjTypePath {
    type Item = ObjTypePathComp;
    type IntoIter = <Vec<ObjTypePathComp> as IntoIterator>::IntoIter;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.components.into_iter()
    }
}

impl std::fmt::Debug for ObjTypePath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.as_slice().fmt(f)
    }
}

impl std::fmt::Display for ObjTypePath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use std::fmt::Write as _;

        let mut iter = self.components.iter();
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

impl From<&str> for ObjTypePath {
    #[inline]
    fn from(component: &str) -> Self {
        Self::new(vec![ObjTypePathComp::Name(component.into())])
    }
}

impl From<ObjTypePathComp> for ObjTypePath {
    #[inline]
    fn from(component: ObjTypePathComp) -> Self {
        Self::new(vec![component])
    }
}

impl std::ops::Div for ObjTypePathComp {
    type Output = ObjTypePath;

    #[inline]
    fn div(self, rhs: ObjTypePathComp) -> Self::Output {
        ObjTypePath::new(vec![self, rhs])
    }
}

impl std::ops::Div<ObjTypePathComp> for ObjTypePath {
    type Output = ObjTypePath;

    #[inline]
    fn div(mut self, rhs: ObjTypePathComp) -> Self::Output {
        self.push(rhs);
        self
    }
}

impl std::ops::Div<&str> for ObjTypePath {
    type Output = ObjTypePath;

    #[inline]
    fn div(mut self, rhs: &str) -> Self::Output {
        self.push(ObjTypePathComp::Name(rhs.into()));
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
