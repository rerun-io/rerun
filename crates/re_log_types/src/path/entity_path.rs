use std::sync::Arc;

use re_string_interner::InternedString;
use re_types_core::SizeBytes;

use crate::{hash::Hash64, EntityPathPart};

// ----------------------------------------------------------------------------

/// A 64 bit hash of [`EntityPath`] with very small risk of collision.
#[derive(Copy, Clone, Eq, PartialOrd, Ord)]
pub struct EntityPathHash(Hash64);

impl re_types_core::SizeBytes for EntityPathHash {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.0.heap_size_bytes()
    }
}

impl EntityPathHash {
    /// Sometimes used as the hash of `None`.
    pub const NONE: EntityPathHash = EntityPathHash(Hash64::ZERO);

    /// From an existing u64. Use this only for data conversions.
    #[inline]
    pub fn from_u64(i: u64) -> Self {
        Self(Hash64::from_u64(i))
    }

    #[inline]
    pub fn hash64(&self) -> u64 {
        self.0.hash64()
    }

    #[inline]
    pub fn is_some(&self) -> bool {
        *self != Self::NONE
    }

    #[inline]
    pub fn is_none(&self) -> bool {
        *self == Self::NONE
    }
}

impl std::hash::Hash for EntityPathHash {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl std::cmp::PartialEq for EntityPathHash {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.0.eq(&other.0)
    }
}

impl nohash_hasher::IsEnabled for EntityPathHash {}

impl std::fmt::Debug for EntityPathHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "EntityPathHash({:016X})", self.hash64())
    }
}

// ----------------------------------------------------------------------------

/// The unique identifier of an entity, e.g. `camera/3/points`
///
/// The entity path is a list of [parts][EntityPathPart] separated by slashes.
/// Each part is a non-empty string, that can contain any character.
/// When written as a string, some characters in the parts need to be escaped with a `\`
/// (only character, numbers, `.`, `-`, `_` does not need escaping).
///
/// See <https://www.rerun.io/docs/concepts/entity-path> for more on entity paths.
///
/// This is basically implemented as a list of strings, but is reference-counted internally, so it is cheap to clone.
/// It also has a precomputed hash and implemented [`nohash_hasher::IsEnabled`],
/// so it is very cheap to use in a [`nohash_hasher::IntMap`] and [`nohash_hasher::IntSet`].
///
/// ```
/// # use re_log_types::EntityPath;
/// assert_eq!(
///     EntityPath::parse_strict(r#"camera/ACME\ Örnöga/points/42"#).unwrap(),
///     EntityPath::new(vec![
///         "camera".into(),
///         "ACME Örnöga".into(),
///         "points".into(),
///         "42".into(),
///     ])
/// );
/// ```
#[derive(Clone, Eq)]
pub struct EntityPath {
    /// precomputed hash
    hash: EntityPathHash,

    // [`Arc`] used for cheap cloning, and to keep down the size of [`EntityPath`].
    // We mostly use the hash for lookups and comparisons anyway!
    parts: Arc<Vec<EntityPathPart>>,
}

impl EntityPath {
    #[inline]
    pub fn root() -> Self {
        Self::from(vec![])
    }

    #[inline]
    pub fn new(parts: Vec<EntityPathPart>) -> Self {
        Self::from(parts)
    }

    /// Treat the file path as one opaque string.
    ///
    /// The file path separators will NOT become splits in the new path.
    /// The returned path will only have one part.
    pub fn from_file_path_as_single_string(file_path: &std::path::Path) -> Self {
        Self::from_single_string(file_path.to_string_lossy().to_string())
    }

    /// Treat the file path as an entity path hierarchy.
    ///
    /// The file path separators will become splits in the new path.
    pub fn from_file_path(file_path: &std::path::Path) -> Self {
        use clean_path::Clean as _;
        Self::new(
            file_path
                .clean()
                .iter()
                .map(|p| EntityPathPart::from(p.to_string_lossy().to_string()))
                .collect(),
        )
    }

    /// Treat the string as one opaque string, NOT splitting on any slashes.
    ///
    /// The given string is expected to be unescaped, i.e. any `\` is treated as a normal character.
    pub fn from_single_string(string: impl Into<InternedString>) -> Self {
        Self::new(vec![EntityPathPart::new(string)])
    }

    #[inline]
    pub fn iter(&self) -> impl DoubleEndedIterator<Item = &EntityPathPart> + ExactSizeIterator {
        self.parts.iter()
    }

    #[inline]
    pub fn last(&self) -> Option<&EntityPathPart> {
        self.parts.last()
    }

    #[inline]
    pub fn as_slice(&self) -> &[EntityPathPart] {
        self.parts.as_slice()
    }

    #[inline]
    pub fn to_vec(&self) -> Vec<EntityPathPart> {
        self.parts.to_vec()
    }

    #[inline]
    pub fn is_root(&self) -> bool {
        self.parts.is_empty()
    }

    /// Is this equals to, or a descendant of, the given path.
    #[inline]
    pub fn starts_with(&self, prefix: &EntityPath) -> bool {
        if self.hash == prefix.hash {
            return true; // optimization!
        }

        prefix.len() <= self.len() && self.iter().zip(prefix.iter()).all(|(a, b)| a == b)
    }

    /// Is this a strict descendant of the given path.
    #[inline]
    pub fn is_descendant_of(&self, other: &EntityPath) -> bool {
        other.len() < self.len() && self.iter().zip(other.iter()).all(|(a, b)| a == b)
    }

    /// Is this a direct child of the other path.
    #[inline]
    pub fn is_child_of(&self, other: &EntityPath) -> bool {
        other.len() + 1 == self.len() && self.iter().zip(other.iter()).all(|(a, b)| a == b)
    }

    /// Number of parts
    #[inline]
    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        self.parts.len()
    }

    #[inline]
    pub fn hash(&self) -> EntityPathHash {
        self.hash
    }

    /// Precomputed 64-bit hash.
    #[inline]
    pub fn hash64(&self) -> u64 {
        self.hash.hash64()
    }

    /// Return [`None`] if root.
    #[must_use]
    pub fn parent(&self) -> Option<Self> {
        self.parts
            .len()
            .checked_sub(1)
            .map(|n_minus_1| Self::new(self.parts[..n_minus_1].to_vec()))
    }

    pub fn join(&self, other: &Self) -> Self {
        self.iter().chain(other.iter()).cloned().collect()
    }

    /// Helper function to iterate over all incremental [`EntityPath`]s from start to end, NOT including start itself.
    ///
    /// For example `incremental_walk("foo", "foo/bar/baz")` returns: `["foo/bar", "foo/bar/baz"]`
    pub fn incremental_walk<'a>(
        start: Option<&'_ EntityPath>,
        end: &'a EntityPath,
    ) -> impl Iterator<Item = EntityPath> + 'a {
        re_tracing::profile_function!();
        if start.map_or(true, |start| end.is_descendant_of(start)) {
            let first_ind = start.map_or(0, |start| start.len() + 1);
            let parts = end.as_slice();
            itertools::Either::Left((first_ind..=end.len()).map(|i| EntityPath::from(&parts[0..i])))
        } else {
            itertools::Either::Right(std::iter::empty())
        }
    }
}

impl SizeBytes for EntityPath {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        0 // NOTE: we assume it's amortized due to the `Arc`
    }
}

impl FromIterator<EntityPathPart> for EntityPath {
    fn from_iter<T: IntoIterator<Item = EntityPathPart>>(parts: T) -> Self {
        Self::new(parts.into_iter().collect())
    }
}

impl From<Vec<EntityPathPart>> for EntityPath {
    #[inline]
    fn from(path: Vec<EntityPathPart>) -> Self {
        Self {
            hash: EntityPathHash(Hash64::hash(&path)),
            parts: Arc::new(path),
        }
    }
}

impl From<&[EntityPathPart]> for EntityPath {
    #[inline]
    fn from(path: &[EntityPathPart]) -> Self {
        Self::from(path.to_vec())
    }
}

impl From<&str> for EntityPath {
    #[inline]
    fn from(path: &str) -> Self {
        EntityPath::parse_forgiving(path)
    }
}

impl From<String> for EntityPath {
    #[inline]
    fn from(path: String) -> Self {
        EntityPath::parse_forgiving(&path)
    }
}

impl From<EntityPath> for String {
    #[inline]
    fn from(path: EntityPath) -> Self {
        path.to_string()
    }
}

impl From<re_types_core::datatypes::EntityPath> for EntityPath {
    #[inline]
    fn from(value: re_types_core::datatypes::EntityPath) -> Self {
        EntityPath::parse_forgiving(&value.0)
    }
}

impl From<&EntityPath> for re_types_core::datatypes::EntityPath {
    #[inline]
    fn from(value: &EntityPath) -> Self {
        Self(value.to_string().into())
    }
}

impl<Idx> std::ops::Index<Idx> for EntityPath
where
    Idx: std::slice::SliceIndex<[EntityPathPart]>,
{
    type Output = Idx::Output;

    fn index(&self, index: Idx) -> &Self::Output {
        &self.parts[index]
    }
}
// ----------------------------------------------------------------------------

use re_types_core::Loggable;

re_types_core::macros::impl_into_cow!(EntityPath);

impl Loggable for EntityPath {
    type Name = re_types_core::ComponentName;

    #[inline]
    fn name() -> Self::Name {
        "rerun.controls.EntityPath".into()
    }

    #[inline]
    fn arrow_datatype() -> arrow2::datatypes::DataType {
        re_types_core::datatypes::Utf8::arrow_datatype()
    }

    fn to_arrow_opt<'a>(
        _data: impl IntoIterator<Item = Option<impl Into<std::borrow::Cow<'a, Self>>>>,
    ) -> re_types_core::SerializationResult<Box<dyn arrow2::array::Array>>
    where
        Self: 'a,
    {
        Err(re_types_core::SerializationError::not_implemented(
            Self::name(),
            "EntityPaths are never nullable, use `to_arrow()` instead",
        ))
    }

    #[inline]
    fn to_arrow<'a>(
        data: impl IntoIterator<Item = impl Into<std::borrow::Cow<'a, Self>>>,
    ) -> re_types_core::SerializationResult<Box<dyn ::arrow2::array::Array>>
    where
        Self: 'a,
    {
        re_types_core::datatypes::Utf8::to_arrow(
            data.into_iter()
                .map(Into::into)
                .map(|ent_path| re_types_core::datatypes::Utf8(ent_path.to_string().into())),
        )
    }

    fn from_arrow(
        array: &dyn ::arrow2::array::Array,
    ) -> re_types_core::DeserializationResult<Vec<Self>> {
        Ok(re_types_core::datatypes::Utf8::from_arrow(array)?
            .into_iter()
            .map(|utf8| Self::from(utf8.to_string()))
            .collect())
    }
}

// ----------------------------------------------------------------------------

#[cfg(feature = "serde")]
impl serde::Serialize for EntityPath {
    #[inline]
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.parts.serialize(serializer)
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for EntityPath {
    #[inline]
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let parts = Vec::<EntityPathPart>::deserialize(deserializer)?;
        Ok(Self::new(parts))
    }
}

// ----------------------------------------------------------------------------

impl std::cmp::PartialEq for EntityPath {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.hash == other.hash // much faster, and low risk of collision
    }
}

impl std::hash::Hash for EntityPath {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.hash.hash(state);
    }
}

impl nohash_hasher::IsEnabled for EntityPath {}

// ----------------------------------------------------------------------------

impl std::cmp::Ord for EntityPath {
    #[inline]
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.parts.cmp(&other.parts)
    }
}

impl std::cmp::PartialOrd for EntityPath {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.parts.cmp(&other.parts))
    }
}

// ----------------------------------------------------------------------------

impl std::fmt::Debug for EntityPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Same as `Display` - since we always prefix paths with a slash, they are easily recognizable.
        write!(f, "{self}")
    }
}

impl std::fmt::Display for EntityPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use std::fmt::Write as _;

        if self.is_root() {
            f.write_char('/')
        } else {
            // We always lead with a slash
            for comp in self.iter() {
                f.write_char('/')?;
                comp.escaped_string().fmt(f)?;
            }
            Ok(())
        }
    }
}

// ----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_incremental_walk() {
        assert_eq!(
            EntityPath::incremental_walk(None, &EntityPath::root()).collect::<Vec<_>>(),
            vec![EntityPath::root()]
        );
        assert_eq!(
            EntityPath::incremental_walk(Some(&EntityPath::root()), &EntityPath::root())
                .collect::<Vec<_>>(),
            vec![]
        );
        assert_eq!(
            EntityPath::incremental_walk(None, &EntityPath::from("foo")).collect::<Vec<_>>(),
            vec![EntityPath::root(), EntityPath::from("foo")]
        );
        assert_eq!(
            EntityPath::incremental_walk(Some(&EntityPath::root()), &EntityPath::from("foo"))
                .collect::<Vec<_>>(),
            vec![EntityPath::from("foo")]
        );
        assert_eq!(
            EntityPath::incremental_walk(None, &EntityPath::from("foo/bar")).collect::<Vec<_>>(),
            vec![
                EntityPath::root(),
                EntityPath::from("foo"),
                EntityPath::from("foo/bar")
            ]
        );
        assert_eq!(
            EntityPath::incremental_walk(
                Some(&EntityPath::from("foo")),
                &EntityPath::from("foo/bar/baz")
            )
            .collect::<Vec<_>>(),
            vec![EntityPath::from("foo/bar"), EntityPath::from("foo/bar/baz")]
        );
    }
}
