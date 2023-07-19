use arrow2_convert::{ArrowDeserialize, ArrowField, ArrowSerialize};

use crate::Component;

/// A number used to specify a specific instance in an entity.
///
/// Each entity can have many component of the same type.
/// These are identified with [`InstanceKey`].
///
/// This is a special component type. All entities has this component, at least implicitly.
///
/// For instance: A point cloud is one entity, and each point is an instance, idenitifed by an [`InstanceKey`].
///
/// ```
/// use re_log_types::InstanceKey;
/// use arrow2_convert::field::ArrowField;
/// use arrow2::datatypes::{DataType, Field};
///
/// assert_eq!(InstanceKey::data_type(), DataType::UInt64);
/// ```
#[derive(
    Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, ArrowField, ArrowSerialize, ArrowDeserialize,
)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[arrow_field(transparent)]
pub struct InstanceKey(pub u64);

impl InstanceKey {
    /// A special value indicating that this [`InstanceKey]` is referring to all instances of an entity,
    /// for example all points in a point cloud entity.
    pub const SPLAT: Self = Self(u64::MAX);

    #[allow(clippy::should_implement_trait)]
    #[inline]
    pub fn from_iter(it: impl IntoIterator<Item = impl Into<Self>>) -> Vec<Self> {
        it.into_iter().map(Into::into).collect::<Vec<_>>()
    }

    /// Are we referring to all instances of the entity (e.g. all points in a point cloud entity)?
    ///
    /// The opposite of [`Self::is_specific`].
    #[inline]
    pub fn is_splat(self) -> bool {
        self == Self::SPLAT
    }

    /// Are we referring to a specific instance of the entity (e.g. a specific point in a point cloud)?
    ///
    /// The opposite of [`Self::is_splat`].
    #[inline]
    pub fn is_specific(self) -> bool {
        self != Self::SPLAT
    }

    /// Returns `None` if splat, otherwise the index.
    #[inline]
    pub fn specific_index(self) -> Option<InstanceKey> {
        self.is_specific().then_some(self)
    }

    /// Creates a new [`InstanceKey`] that identifies a 2d coordinate.
    pub fn from_2d_image_coordinate([x, y]: [u32; 2], image_width: u64) -> Self {
        Self((x as u64) + (y as u64) * image_width)
    }

    /// Retrieves 2d image coordinates (x, y) encoded in an instance key
    pub fn to_2d_image_coordinate(self, image_width: u64) -> [u32; 2] {
        [(self.0 % image_width) as u32, (self.0 / image_width) as u32]
    }
}

impl std::fmt::Debug for InstanceKey {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_splat() {
            "splat".fmt(f)
        } else {
            self.0.fmt(f)
        }
    }
}

impl std::fmt::Display for InstanceKey {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_splat() {
            "splat".fmt(f)
        } else {
            self.0.fmt(f)
        }
    }
}

impl From<u64> for InstanceKey {
    #[inline]
    fn from(value: u64) -> Self {
        Self(value)
    }
}

impl Component for InstanceKey {
    #[inline]
    fn legacy_name() -> crate::ComponentName {
        "rerun.instance_key".into()
    }
}

impl From<re_types::components::InstanceKey> for InstanceKey {
    fn from(other: re_types::components::InstanceKey) -> Self {
        Self(other.0)
    }
}

// Can't use
impl re_types::Loggable for InstanceKey {
    type Name = re_types::ComponentName;

    #[inline]
    fn name() -> Self::Name {
        Self::legacy_name().as_str().into()
    }

    fn to_arrow_datatype() -> arrow2::datatypes::DataType {
        Self::field().data_type().clone()
    }

    fn try_to_arrow_opt<'a>(
        data: impl IntoIterator<Item = Option<impl Into<std::borrow::Cow<'a, Self>>>>,
        _extension_wrapper: Option<&str>,
    ) -> re_types::SerializationResult<Box<dyn arrow2::array::Array>>
    where
        Self: Clone + 'a,
    {
        // TODO(jleibs) What do we do with the extension_wrapper?

        let input = data.into_iter().map(|datum| {
            let datum: Option<::std::borrow::Cow<'a, Self>> = datum.map(Into::into);
            datum.map(|d| d.into_owned())
        });

        // TODO(jleibs): Why can't we feed input directly into try_into_arrow?
        let vec: Vec<_> = input.collect();

        let arrow = arrow2_convert::serialize::TryIntoArrow::try_into_arrow(vec.iter())
            .map_err(|e| re_types::SerializationError::ArrowConvertFailure(e.to_string()))?;

        Ok(arrow)
    }

    fn try_from_arrow_opt(
        data: &dyn arrow2::array::Array,
    ) -> re_types::DeserializationResult<Vec<Option<Self>>>
    where
        Self: Sized,
    {
        use arrow2_convert::deserialize::arrow_array_deserialize_iterator;

        // TODO(jleibs): These collects are going to be problematic
        let native = arrow_array_deserialize_iterator(data)
            .map_err(|e| re_types::DeserializationError::ArrowConvertFailure(e.to_string()))?
            .collect();

        Ok(native)
    }
}

impl re_types::Component for InstanceKey {}

impl<'a> From<InstanceKey> for ::std::borrow::Cow<'a, InstanceKey> {
    #[inline]
    fn from(value: InstanceKey) -> Self {
        std::borrow::Cow::Owned(value)
    }
}

impl<'a> From<&'a InstanceKey> for ::std::borrow::Cow<'a, InstanceKey> {
    #[inline]
    fn from(value: &'a InstanceKey) -> Self {
        std::borrow::Cow::Borrowed(value)
    }
}
