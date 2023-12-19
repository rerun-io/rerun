use re_types_core::{Loggable, SizeBytes};

// ---

/// Explicit number of instances in a [`crate::DataRow`].
///
/// Component batches in that row should have a length of either this number, zero (clear) or one (splat).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(transparent)]
pub struct NumInstances(pub u32);

impl NumInstances {
    #[inline]
    pub fn get(self) -> u32 {
        self.0
    }
}

impl From<NumInstances> for u32 {
    #[inline]
    fn from(val: NumInstances) -> Self {
        val.0
    }
}

impl From<u32> for NumInstances {
    #[inline]
    fn from(value: u32) -> Self {
        Self(value)
    }
}

impl SizeBytes for NumInstances {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        0
    }

    #[inline]
    fn is_pod() -> bool {
        true
    }
}

re_types_core::macros::impl_into_cow!(NumInstances);

impl Loggable for NumInstances {
    type Name = re_types_core::ComponentName;

    #[inline]
    fn name() -> Self::Name {
        "rerun.controls.NumInstances".into()
    }

    #[inline]
    fn arrow_datatype() -> arrow2::datatypes::DataType {
        re_types_core::datatypes::UInt32::arrow_datatype()
    }

    fn to_arrow_opt<'a>(
        _data: impl IntoIterator<Item = Option<impl Into<std::borrow::Cow<'a, Self>>>>,
    ) -> re_types_core::SerializationResult<Box<dyn arrow2::array::Array>>
    where
        Self: 'a,
    {
        Err(re_types_core::SerializationError::not_implemented(
            Self::name(),
            "NumInstances is never nullable, use `to_arrow()` instead",
        ))
    }

    #[inline]
    fn to_arrow<'a>(
        data: impl IntoIterator<Item = impl Into<std::borrow::Cow<'a, Self>>>,
    ) -> re_types_core::SerializationResult<Box<dyn ::arrow2::array::Array>>
    where
        Self: 'a,
    {
        use re_types_core::datatypes::UInt32;
        UInt32::to_arrow(data.into_iter().map(Into::into).map(|c| UInt32(c.0)))
    }

    fn from_arrow(
        array: &dyn ::arrow2::array::Array,
    ) -> re_types_core::DeserializationResult<Vec<Self>> {
        use re_types_core::datatypes::UInt32;
        Ok(UInt32::from_arrow(array)?
            .into_iter()
            .map(|v| Self(v.0))
            .collect())
    }
}
