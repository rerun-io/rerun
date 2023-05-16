use crate::path::EntityPathPart;

/// `camera / "left" / points / #42`
///
/// Wrapped by [`crate::EntityPath`] together with a hash.
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct EntityPathImpl {
    parts: Vec<EntityPathPart>,
}

impl EntityPathImpl {
    #[inline]
    pub fn root() -> Self {
        Self { parts: vec![] }
    }

    #[inline]
    pub fn new(parts: Vec<EntityPathPart>) -> Self {
        Self { parts }
    }

    #[inline]
    pub fn as_slice(&self) -> &[EntityPathPart] {
        self.parts.as_slice()
    }

    #[inline]
    pub fn is_root(&self) -> bool {
        self.parts.is_empty()
    }

    /// Number of components
    #[inline]
    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        self.parts.len()
    }

    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = &EntityPathPart> {
        self.parts.iter()
    }

    #[inline]
    pub fn last(&self) -> Option<&EntityPathPart> {
        self.parts.last()
    }

    #[inline]
    pub fn push(&mut self, comp: EntityPathPart) {
        self.parts.push(comp);
    }

    /// Return [`None`] if root.
    #[must_use]
    pub fn parent(&self) -> Option<Self> {
        if self.parts.is_empty() {
            None
        } else {
            Some(Self::new(self.parts[..(self.parts.len() - 1)].to_vec()))
        }
    }
}

// ----------------------------------------------------------------------------

impl<'a, It> From<It> for EntityPathImpl
where
    It: Iterator<Item = &'a EntityPathPart>,
{
    fn from(path: It) -> Self {
        Self::new(path.cloned().collect())
    }
}

// ----------------------------------------------------------------------------

impl std::fmt::Debug for EntityPathImpl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Show the nicely formatted entity path, but surround with quotes and escape
        // troublesome characters such as back-slashes and newlines.
        write!(f, "{:?}", self.to_string())
    }
}

impl std::fmt::Display for EntityPathImpl {
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
