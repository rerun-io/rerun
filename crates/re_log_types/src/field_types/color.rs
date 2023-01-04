use arrow2::{array::TryPush, datatypes::DataType};
use arrow2_convert::{
    arrow_enable_vec_for_type, deserialize::ArrowDeserialize, field::ArrowField,
    serialize::ArrowSerialize,
};

use crate::msg_bundle::Component;

/// An RGBA color tuple.
///
/// ```
/// use re_log_types::field_types::ColorRGBA;
/// use arrow2_convert::field::ArrowField;
/// use arrow2::datatypes::{DataType, Field};
///
/// assert_eq!(ColorRGBA::data_type(), DataType::UInt32);
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, derive_more::From, derive_more::Into)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct ColorRGBA(pub u32);

impl ColorRGBA {
    pub fn to_array(&self) -> [u8; 4] {
        [
            (self.0 >> 24) as u8,
            (self.0 >> 16) as u8,
            (self.0 >> 8) as u8,
            self.0 as u8,
        ]
    }
}

arrow_enable_vec_for_type!(ColorRGBA);

impl ArrowField for ColorRGBA {
    type Type = Self;
    fn data_type() -> DataType {
        <u32 as ArrowField>::data_type()
    }
}

impl ArrowSerialize for ColorRGBA {
    type MutableArrayType = <u32 as ArrowSerialize>::MutableArrayType;

    #[inline]
    fn new_array() -> Self::MutableArrayType {
        Self::MutableArrayType::default()
    }

    #[inline]
    fn arrow_serialize(v: &Self, array: &mut Self::MutableArrayType) -> arrow2::error::Result<()> {
        array.try_push(Some(v.0))
    }
}

impl ArrowDeserialize for ColorRGBA {
    type ArrayType = <u32 as ArrowDeserialize>::ArrayType;

    #[inline]
    fn arrow_deserialize(
        v: <&Self::ArrayType as IntoIterator>::Item,
    ) -> Option<<Self as ArrowField>::Type> {
        <u32 as ArrowDeserialize>::arrow_deserialize(v).map(ColorRGBA)
    }
}

impl Component for ColorRGBA {
    fn name() -> crate::ComponentName {
        "rerun.colorrgba".into()
    }
}

#[cfg(feature = "ecolor")]
impl From<ColorRGBA> for ecolor::Color32 {
    fn from(color: ColorRGBA) -> Self {
        let [r, g, b, a] = color.to_array();
        Self::from_rgba_premultiplied(r, g, b, a)
    }
}

#[test]
fn test_colorrgba_roundtrip() {
    use arrow2::array::Array;
    use arrow2_convert::{deserialize::TryIntoCollection, serialize::TryIntoArrow};

    let colors_in = vec![ColorRGBA(0u32), ColorRGBA(255u32)];
    let array: Box<dyn Array> = colors_in.try_into_arrow().unwrap();
    let colors_out: Vec<ColorRGBA> = TryIntoCollection::try_into_collection(array).unwrap();
    assert_eq!(colors_in, colors_out);
}
