/// An RGBA color tuple with unmultiplied/separate alpha,
/// in sRGB gamma space with linear alpha.
///
/// ```
/// use re_types::components::ColorRGBA;
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
    bytemuck::Pod,
    bytemuck::Zeroable,
    arrow2_convert::ArrowField,
    arrow2_convert::ArrowSerialize,
    arrow2_convert::ArrowDeserialize,
)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[arrow_field(transparent)]
#[repr(transparent)]
pub struct LegacyColor(pub u32);

impl From<LegacyColor> for re_types::components::Color {
    fn from(val: LegacyColor) -> Self {
        re_types::components::Color(val.0)
    }
}

impl From<re_types::components::Color> for LegacyColor {
    fn from(val: re_types::components::Color) -> Self {
        LegacyColor(val.0)
    }
}

impl LegacyColor {
    #[inline]
    pub fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Self::from([r, g, b, 255])
    }

    #[inline]
    pub fn from_unmultiplied_rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self::from([r, g, b, a])
    }

    #[inline]
    pub fn to_array(self) -> [u8; 4] {
        [
            (self.0 >> 24) as u8,
            (self.0 >> 16) as u8,
            (self.0 >> 8) as u8,
            self.0 as u8,
        ]
    }
}

impl From<[u8; 4]> for LegacyColor {
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

impl re_log_types::LegacyComponent for LegacyColor {
    #[inline]
    fn legacy_name() -> re_log_types::ComponentName {
        "rerun.colorrgba".into()
    }
}

#[cfg(feature = "ecolor")]
impl From<LegacyColor> for ecolor::Color32 {
    fn from(color: LegacyColor) -> Self {
        let [r, g, b, a] = color.to_array();
        Self::from_rgba_premultiplied(r, g, b, a)
    }
}

re_log_types::component_legacy_shim!(LegacyColor);

#[test]
fn test_colorrgba_roundtrip() {
    use arrow2::array::Array;
    use arrow2_convert::{deserialize::TryIntoCollection, serialize::TryIntoArrow};

    let colors_in = vec![LegacyColor(0u32), LegacyColor(255u32)];
    let array: Box<dyn Array> = colors_in.try_into_arrow().unwrap();
    let colors_out: Vec<LegacyColor> = TryIntoCollection::try_into_collection(array).unwrap();
    assert_eq!(colors_in, colors_out);
}
