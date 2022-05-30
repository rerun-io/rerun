use rr_string_interner::InternedString;

// ----------------------------------------------------------------------------

/// A path to a specific piece of data (e.g. a single `f32`).
#[derive(Clone, Debug, Default, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct DataPath(pub Vec<DataPathComponent>);

impl DataPath {
    #[inline]
    pub fn is_root(&self) -> bool {
        self.0.is_empty()
    }

    pub fn parent(&self) -> Self {
        let mut path = self.0.clone();
        path.pop();
        Self(path)
    }

    pub fn sibling(&self, last_comp: impl Into<DataPathComponent>) -> Self {
        let mut path = self.0.clone();
        path.pop(); // TODO: handle root?
        path.push(last_comp.into());
        Self(path)
    }
}

impl std::fmt::Display for DataPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use std::fmt::Write as _;

        f.write_char('/')?;
        for (i, comp) in self.0.iter().enumerate() {
            comp.fmt(f)?;
            if i + 1 != self.0.len() {
                f.write_char('/')?;
            }
        }
        Ok(())
    }
}

impl From<&str> for DataPath {
    #[inline]
    fn from(component: &str) -> Self {
        Self(vec![component.into()])
    }
}
// ----------------------------------------------------------------------------

impl From<DataPathComponent> for DataPath {
    #[inline]
    fn from(component: DataPathComponent) -> Self {
        Self(vec![component])
    }
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum DataPathComponent {
    /// Struct member. Each member can have a different type.
    String(InternedString),

    /// Array/table/map member. Each member must be of the same type (homogenous).
    Index(Index),
}

impl std::fmt::Display for DataPathComponent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::String(string) => f.write_str(string),
            Self::Index(index) => index.fmt(f),
        }
    }
}

impl From<&str> for DataPathComponent {
    #[inline]
    fn from(comp: &str) -> Self {
        Self::String(comp.into())
    }
}

// ----------------------------------------------------------------------------

impl std::ops::Div for DataPathComponent {
    type Output = DataPath;

    #[inline]
    fn div(self, rhs: DataPathComponent) -> Self::Output {
        DataPath(vec![self, rhs])
    }
}

impl std::ops::Div<DataPathComponent> for DataPath {
    type Output = DataPath;

    #[inline]
    fn div(mut self, rhs: DataPathComponent) -> Self::Output {
        self.0.push(rhs);
        self
    }
}

impl std::ops::Div<Index> for DataPath {
    type Output = DataPath;

    #[inline]
    fn div(mut self, rhs: Index) -> Self::Output {
        self.0.push(DataPathComponent::Index(rhs));
        self
    }
}

impl std::ops::Div<Index> for &DataPath {
    type Output = DataPath;

    #[inline]
    fn div(self, rhs: Index) -> Self::Output {
        self.clone() / rhs
    }
}

impl std::ops::Div<DataPathComponent> for &DataPath {
    type Output = DataPath;

    #[inline]
    fn div(self, rhs: DataPathComponent) -> Self::Output {
        self.clone() / rhs
    }
}

impl std::ops::Div<&'static str> for DataPath {
    type Output = DataPath;

    #[inline]
    fn div(mut self, rhs: &'static str) -> Self::Output {
        self.0.push(DataPathComponent::String(rhs.into()));
        self
    }
}

impl std::ops::Div<&'static str> for &DataPath {
    type Output = DataPath;

    #[inline]
    fn div(self, rhs: &'static str) -> Self::Output {
        self.clone() / rhs
    }
}

// ----------------------------------------------------------------------------

/// The key of a table.
#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum Index {
    /// For arrays, assumed to be dense (0, 1, 2, …).
    Sequence(u64),

    /// X,Y pixel coordinates, from top left.
    Pixel([u64; 2]),

    /// Any integer, e.g. a hash or an arbitrary identifier.
    Integer(i128),

    /// UUID/GUID
    Uuid(uuid::Uuid),

    /// Anything goes.
    String(String),
}

impl std::fmt::Display for Index {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Sequence(seq) => format!("#{seq}").fmt(f),
            Self::Pixel([x, y]) => format!("[{x}, {y}]").fmt(f),
            Self::Integer(value) => value.fmt(f),
            Self::Uuid(value) => value.fmt(f),
            Self::String(value) => format!("{value:?}").fmt(f), // put it in quotes
        }
    }
}

crate::impl_into_enum!(String, Index, String);
