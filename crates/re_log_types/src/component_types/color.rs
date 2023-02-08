use arrow2_convert::{ArrowDeserialize, ArrowField, ArrowSerialize};

use crate::msg_bundle::Component;

/// An RGBA color tuple.
///
/// ```
/// use re_log_types::component_types::ColorRGBA;
/// use arrow2_convert::field::ArrowField;
/// use arrow2::datatypes::{DataType, Field};
///
/// assert_eq!(ColorRGBA::data_type(), DataType::UInt32);
/// ```
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    derive_more::From,
    derive_more::Into,
    ArrowField,
    ArrowSerialize,
    ArrowDeserialize,
)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[arrow_field(transparent)]
pub struct ColorRGBA(pub u32);

impl ColorRGBA {
    #[inline]
    pub fn to_array(&self) -> [u8; 4] {
        [
            (self.0 >> 24) as u8,
            (self.0 >> 16) as u8,
            (self.0 >> 8) as u8,
            self.0 as u8,
        ]
    }
}

impl From<[u8; 4]> for ColorRGBA {
    #[inline]
    fn from(bytes: [u8; 4]) -> Self {
        Self(
            (bytes[0] as u32) << 24
                | (bytes[1] as u32) << 16
                | (bytes[2] as u32) << 8
                | (bytes[3] as u32),
        )
    }
}

impl Component for ColorRGBA {
    #[inline]
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
