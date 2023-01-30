use crate::path::EntityPathComponent;

/// `camera / "left" / points / #42`
///
/// Wrapped by [`crate::EntityPath`] together with a hash.
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct EntityPathImpl {
    components: Vec<EntityPathComponent>,
}

impl EntityPathImpl {
    #[inline]
    pub fn root() -> Self {
        Self { components: vec![] }
    }

    #[inline]
    pub fn new(components: Vec<EntityPathComponent>) -> Self {
        Self { components }
    }

    #[inline]
    pub fn as_slice(&self) -> &[EntityPathComponent] {
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
    pub fn iter(&self) -> impl Iterator<Item = &EntityPathComponent> {
        self.components.iter()
    }

    #[inline]
    pub fn push(&mut self, comp: EntityPathComponent) {
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

impl<'a, It> From<It> for EntityPathImpl
where
    It: Iterator<Item = &'a EntityPathComponent>,
{
    fn from(path: It) -> Self {
        Self::new(path.cloned().collect())
    }
}

// ----------------------------------------------------------------------------

impl std::fmt::Debug for EntityPathImpl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "EntityPath({self})")
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
