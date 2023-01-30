use crate::path::ObjPathComp;

/// `camera / "left" / points / #42`
///
/// Wrapped by [`crate::ObjPath`] together with a hash.
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct ObjPathImpl {
    components: Vec<ObjPathComp>,
}

impl ObjPathImpl {
    #[inline]
    pub fn root() -> Self {
        Self { components: vec![] }
    }

    #[inline]
    pub fn new(components: Vec<ObjPathComp>) -> Self {
        Self { components }
    }

    #[inline]
    pub fn as_slice(&self) -> &[ObjPathComp] {
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
    pub fn iter(&self) -> impl Iterator<Item = &ObjPathComp> {
        self.components.iter()
    }

    #[inline]
    pub fn push(&mut self, comp: ObjPathComp) {
        self.components.push(comp);
    }

    /// Return [`None`] if root.
    #[must_use]
    pub fn parent(&self) -> Option<Self> {
        if self.components.is_empty() {
            None
        } else {
            Some(Self::new(
                self.components[..(self.components.len() - 1)].to_vec(),
            ))
        }
    }
}

// ----------------------------------------------------------------------------

impl<'a, It> From<It> for ObjPathImpl
where
    It: Iterator<Item = &'a ObjPathComp>,
{
    fn from(path: It) -> Self {
        Self::new(path.cloned().collect())
    }
}

// ----------------------------------------------------------------------------

impl std::fmt::Debug for ObjPathImpl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ObjPath({self})")
    }
}

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
