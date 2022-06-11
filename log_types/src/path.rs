use rr_string_interner::InternedString;

// ----------------------------------------------------------------------------

#[inline]
fn double_hash(value: impl std::hash::Hash + Copy) -> [u64; 2] {
    [hash_with_seed(value, 123), hash_with_seed(value, 456)]
}

/// Hash the given value.
#[inline]
fn hash_with_seed(value: impl std::hash::Hash, seed: u128) -> u64 {
    use std::hash::Hasher as _;
    let mut hasher = ahash::AHasher::new_with_keys(666, seed);
    value.hash(&mut hasher);
    hasher.finish()
}

#[derive(Copy, Clone, Debug, Eq, PartialOrd, Ord)]
pub struct DataPathHash([u64; 2]);

impl DataPathHash {
    fn new(components: &[DataPathComponent]) -> Self {
        Self(double_hash(components))
    }

    #[inline]
    pub fn hash64(&self) -> u64 {
        self.0[0]
    }
}

impl std::hash::Hash for DataPathHash {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        state.write_u64(self.0[0]);
    }
}

impl std::cmp::PartialEq for DataPathHash {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl nohash_hasher::IsEnabled for DataPathHash {}

// ----------------------------------------------------------------------------

/// A path to a specific piece of data (e.g. a single `f32`).
#[derive(Clone, Debug, Eq, PartialOrd, Ord)]
pub struct DataPath {
    components: Vec<DataPathComponent>,
    hash: DataPathHash,
}

impl DataPath {
    pub fn new(components: Vec<DataPathComponent>) -> Self {
        let hash = DataPathHash::new(&components);
        Self { components, hash }
    }

    #[inline]
    pub fn as_slice(&self) -> &[DataPathComponent] {
        self.components.as_slice()
    }

    /// Precomputed hash.
    #[inline]
    pub fn hash(&self) -> DataPathHash {
        self.hash
    }

    /// Precomputed hash.
    #[inline]
    pub fn hash64(&self) -> u64 {
        self.hash.0[0]
    }

    #[inline]
    pub fn is_root(&self) -> bool {
        self.components.is_empty()
    }

    pub fn starts_with(&self, prefix: &[DataPathComponent]) -> bool {
        self.components.starts_with(prefix)
    }

    #[inline]
    pub fn get(&self, i: usize) -> Option<&DataPathComponent> {
        self.components.get(i)
    }

    pub fn parent(&self) -> Self {
        let mut path = self.components.clone();
        path.pop();
        Self::new(path)
    }

    pub fn sibling(&self, last_comp: impl Into<DataPathComponent>) -> Self {
        let mut path = self.components.clone();
        path.pop(); // TODO: handle root?
        path.push(last_comp.into());
        Self::new(path)
    }

    pub fn push(&mut self, component: DataPathComponent) {
        // TODO(emilk): optimize DataPath construction.
        // This is quite slow, but we only do this in rare circumstances, so it is ok for now.
        let mut components = std::mem::take(&mut self.components);
        components.push(component);
        *self = Self::new(components);
    }

    pub fn to_type_path(&self) -> TypePath {
        TypePath::new(
            self.components
                .iter()
                .map(DataPathComponent::to_type_path_component)
                .collect(),
        )
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for DataPath {
    #[inline]
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.as_slice().serialize(serializer)
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for DataPath {
    #[inline]
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        <Vec<DataPathComponent>>::deserialize(deserializer).map(DataPath::new)
    }
}

impl std::cmp::PartialEq for DataPath {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.hash == other.hash // much faster, and low chance of collision
    }
}

impl std::hash::Hash for DataPath {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        state.write_u64(self.hash.0[0]);
    }
}

impl nohash_hasher::IsEnabled for DataPath {}

impl std::ops::Deref for DataPath {
    type Target = [DataPathComponent];

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.components
    }
}

impl IntoIterator for DataPath {
    type Item = DataPathComponent;
    type IntoIter = <Vec<DataPathComponent> as IntoIterator>::IntoIter;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.components.into_iter()
    }
}

impl std::fmt::Display for DataPath {
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

impl From<&str> for DataPath {
    #[inline]
    fn from(component: &str) -> Self {
        Self::new(vec![component.into()])
    }
}

impl From<DataPathComponent> for DataPath {
    #[inline]
    fn from(component: DataPathComponent) -> Self {
        Self::new(vec![component])
    }
}

// ----------------------------------------------------------------------------

#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum DataPathComponent {
    /// Struct member. Each member can have a different type.
    String(InternedString),

    /// Array/table/map member. Each member must be of the same type (homogenous).
    Index(Index),
}

impl DataPathComponent {
    pub fn to_type_path_component(&self) -> TypePathComponent {
        match self {
            Self::String(name) => TypePathComponent::String(*name),
            Self::Index(_) => TypePathComponent::Index,
        }
    }
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
        DataPath::new(vec![self, rhs])
    }
}

impl std::ops::Div<DataPathComponent> for DataPath {
    type Output = DataPath;

    #[inline]
    fn div(mut self, rhs: DataPathComponent) -> Self::Output {
        self.push(rhs);
        self
    }
}

impl std::ops::Div<Index> for DataPath {
    type Output = DataPath;

    #[inline]
    fn div(mut self, rhs: Index) -> Self::Output {
        self.push(DataPathComponent::Index(rhs));
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
        self.push(DataPathComponent::String(rhs.into()));
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

    /// Used as the last index when logging a batch of data.
    Placeholder, // TODO: `DataPathComponent::IndexPlaceholder` instead?
}

impl std::fmt::Display for Index {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Sequence(seq) => format!("#{seq}").fmt(f),
            Self::Pixel([x, y]) => format!("[{x}, {y}]").fmt(f),
            Self::Integer(value) => value.fmt(f),
            Self::Uuid(value) => value.fmt(f),
            Self::String(value) => format!("{value:?}").fmt(f), // put it in quotes
            Self::Placeholder => "*".fmt(f),                    // put it in quotes
        }
    }
}

crate::impl_into_enum!(String, Index, String);

// ----------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum TypePathComponent {
    /// Struct member
    String(InternedString),

    /// Table (array/map) member.
    /// Tables are homogenous, so it is the same type path for all.
    Index,
}

impl std::fmt::Display for TypePathComponent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::String(string) => f.write_str(string),
            Self::Index => f.write_str("*"),
        }
    }
}

/// Like [`DataPath`], but without any specific indices.
#[derive(Clone, Debug, PartialOrd, Ord, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct TypePath {
    components: im::Vector<TypePathComponent>,
}

impl TypePath {
    #[inline]
    pub fn new(components: im::Vector<TypePathComponent>) -> Self {
        Self { components }
    }

    #[inline]
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &TypePathComponent> {
        self.components.iter()
    }

    #[inline]
    pub fn last(&self) -> Option<&TypePathComponent> {
        self.components.last()
    }

    #[must_use]
    pub fn parent(&self) -> Self {
        let mut components = self.components.clone();
        components.pop_back();
        Self::new(components)
    }

    #[must_use]
    pub fn sibling(&self, name: &str) -> TypePath {
        let mut components = self.components.clone();
        components.pop_back();
        components.push_back(TypePathComponent::String(name.into()));
        Self::new(components)
    }

    pub fn push(&mut self, comp: TypePathComponent) {
        self.components.push_back(comp);
    }
}

impl<'a> IntoIterator for &'a TypePath {
    type Item = &'a TypePathComponent;
    type IntoIter = im::vector::Iter<'a, TypePathComponent>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.components.iter()
    }
}

impl IntoIterator for TypePath {
    type Item = TypePathComponent;
    type IntoIter = <im::Vector<TypePathComponent> as IntoIterator>::IntoIter;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.components.into_iter()
    }
}

impl std::fmt::Display for TypePath {
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

impl From<&str> for TypePath {
    #[inline]
    fn from(component: &str) -> Self {
        Self::new(im::vector![TypePathComponent::String(component.into())])
    }
}

impl From<TypePathComponent> for TypePath {
    #[inline]
    fn from(component: TypePathComponent) -> Self {
        Self::new(im::vector![component])
    }
}

impl std::ops::Div for TypePathComponent {
    type Output = TypePath;

    #[inline]
    fn div(self, rhs: TypePathComponent) -> Self::Output {
        TypePath::new(im::vector![self, rhs])
    }
}

impl std::ops::Div<TypePathComponent> for TypePath {
    type Output = TypePath;

    #[inline]
    fn div(mut self, rhs: TypePathComponent) -> Self::Output {
        self.push(rhs);
        self
    }
}

impl std::ops::Div<&'static str> for TypePath {
    type Output = TypePath;

    #[inline]
    fn div(mut self, rhs: &'static str) -> Self::Output {
        self.push(TypePathComponent::String(rhs.into()));
        self
    }
}

impl std::ops::Div<&'static str> for &TypePath {
    type Output = TypePath;

    #[inline]
    fn div(self, rhs: &'static str) -> Self::Output {
        self.clone() / rhs
    }
}
