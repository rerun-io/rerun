use arrow2_convert::{ArrowDeserialize, ArrowField, ArrowSerialize};

use super::FixedSizeArrayField;
use crate::msg_bundle::Component;

// --- UVec2D ---

/// An unsigned 32bit vector in 2D space.
///
/// ```
/// # use re_log_types::field_types::UVec2D;
/// # use arrow2_convert::field::ArrowField;
/// # use arrow2::datatypes::{DataType, Field};
/// assert_eq!(
///     UVec2D::data_type(),
///     DataType::FixedSizeList(
///         Box::new(Field::new("item", DataType::Uint32, false)),
///         2
///     )
/// );
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, ArrowField, ArrowSerialize, ArrowDeserialize)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[arrow_field(transparent)]
pub struct UVec2D(#[arrow_field(type = "FixedSizeArrayField<u32,2>")] pub [u32; 2]);

impl UVec2D {
    #[inline]
    pub fn x(&self) -> u32 {
        self.0[0]
    }

    #[inline]
    pub fn y(&self) -> u32 {
        self.0[1]
    }
}

impl From<[u32; 2]> for UVec2D {
    #[inline]
    fn from(v: [u32; 2]) -> Self {
        Self(v)
    }
}

impl<Idx> std::ops::Index<Idx> for UVec2D
where
    Idx: std::slice::SliceIndex<[u32]>,
{
    type Output = Idx::Output;

    #[inline]
    fn index(&self, index: Idx) -> &Self::Output {
        &self.0[index]
    }
}

impl Component for UVec2D {
    fn name() -> crate::ComponentName {
        "rerun.uvec2d".into()
    }
}

#[cfg(feature = "glam")]
impl From<UVec2D> for glam::UVec2 {
    #[inline]
    fn from(v: UVec2D) -> Self {
        Self::from_slice(&v.0)
    }
}

#[cfg(feature = "glam")]
impl From<glam::UVec2> for UVec2D {
    #[inline]
    fn from(v: glam::UVec2) -> Self {
        Self(v.to_array())
    }
}
