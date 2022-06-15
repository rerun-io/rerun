use crate::{
    path::{Index, ObjPathComp},
    ObjPath,
};

use super::obj_path::ObjPathComponentRef;

/// A path to a specific piece of data (e.g. a single `f32`).
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ObjPathBuilder {
    components: Vec<ObjPathComp>,
}

impl ObjPathBuilder {
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

    pub fn starts_with(&self, prefix: &[ObjPathComp]) -> bool {
        self.components.starts_with(prefix)
    }

    #[inline]
    pub fn get(&self, i: usize) -> Option<&ObjPathComp> {
        self.components.get(i)
    }

    #[must_use]
    pub fn parent(&self) -> Self {
        Self::new(self.components[0..self.components.len() - 1].to_vec())
    }

    pub fn push(&mut self, component: ObjPathComp) {
        // TODO(emilk): optimize DataPath construction.
        // This is quite slow, but we only do this in rare circumstances, so it is ok for now.
        let mut components = std::mem::take(&mut self.components);
        components.push(component);
        *self = Self::new(components);
    }
}

impl From<&ObjPath> for ObjPathBuilder {
    #[inline]
    fn from(path: &ObjPath) -> Self {
        Self::new(
            path.iter()
                .map(|comp| match comp {
                    ObjPathComponentRef::String(name) => ObjPathComp::String(*name),
                    ObjPathComponentRef::IndexPlaceholder => ObjPathComp::Index(Index::Placeholder),
                    ObjPathComponentRef::Index(index) => ObjPathComp::Index(index.clone()),
                })
                .collect(),
        )
    }
}

impl From<&str> for ObjPathBuilder {
    #[inline]
    fn from(component: &str) -> Self {
        Self::new(vec![component.into()])
    }
}

impl From<ObjPathComp> for ObjPathBuilder {
    #[inline]
    fn from(component: ObjPathComp) -> Self {
        Self::new(vec![component])
    }
}

impl std::ops::Deref for ObjPathBuilder {
    type Target = [ObjPathComp];

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.components
    }
}

impl IntoIterator for ObjPathBuilder {
    type Item = ObjPathComp;
    type IntoIter = <Vec<ObjPathComp> as IntoIterator>::IntoIter;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.components.into_iter()
    }
}

impl std::fmt::Display for ObjPathBuilder {
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

// ----------------------------------------------------------------------------

impl std::ops::Div for ObjPathComp {
    type Output = ObjPathBuilder;

    #[inline]
    fn div(self, rhs: ObjPathComp) -> Self::Output {
        ObjPathBuilder::new(vec![self, rhs])
    }
}

impl std::ops::Div<ObjPathComp> for ObjPathBuilder {
    type Output = ObjPathBuilder;

    #[inline]
    fn div(mut self, rhs: ObjPathComp) -> Self::Output {
        self.push(rhs);
        self
    }
}

impl std::ops::Div<Index> for ObjPathBuilder {
    type Output = ObjPathBuilder;

    #[inline]
    fn div(mut self, rhs: Index) -> Self::Output {
        self.push(ObjPathComp::Index(rhs));
        self
    }
}

impl std::ops::Div<Index> for &ObjPathBuilder {
    type Output = ObjPathBuilder;

    #[inline]
    fn div(self, rhs: Index) -> Self::Output {
        self.clone() / rhs
    }
}

impl std::ops::Div<ObjPathComp> for &ObjPathBuilder {
    type Output = ObjPathBuilder;

    #[inline]
    fn div(self, rhs: ObjPathComp) -> Self::Output {
        self.clone() / rhs
    }
}

impl std::ops::Div<&'static str> for ObjPathBuilder {
    type Output = ObjPathBuilder;

    #[inline]
    fn div(mut self, rhs: &'static str) -> Self::Output {
        self.push(ObjPathComp::String(rhs.into()));
        self
    }
}

impl std::ops::Div<&'static str> for &ObjPathBuilder {
    type Output = ObjPathBuilder;

    #[inline]
    fn div(self, rhs: &'static str) -> Self::Output {
        self.clone() / rhs
    }
}
