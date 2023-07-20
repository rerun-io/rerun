/// An RGBA color tuple with unmultiplied/separate alpha,
/// in sRGB gamma space with linear alpha.
///
/// ```
/// use re_components::ColorRGBA;
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
pub struct ColorRGBA(pub u32);

impl ColorRGBA {
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

impl re_log_types::LegacyComponent for ColorRGBA {
    #[inline]
    fn legacy_name() -> re_log_types::ComponentName {
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

impl ColorRGBA {}

impl re_types::Loggable for ColorRGBA {
    type Name = re_types::ComponentName;
    type Item<'a> = <&'a <Self as arrow2_convert::deserialize::ArrowDeserialize>::ArrayType as IntoIterator>::Item;
    type IterItem<'a> =
        <&'a <Self as arrow2_convert::deserialize::ArrowDeserialize>::ArrayType as IntoIterator>::IntoIter;

    #[inline]
    fn name() -> Self::Name {
        <Self as re_log_types::LegacyComponent>::legacy_name()
            .as_str()
            .into()
    }
    fn to_arrow_datatype() -> arrow2::datatypes::DataType {
        <Self as re_log_types::LegacyComponent>::field()
            .data_type()
            .clone()
    }
    fn try_to_arrow_opt<'a>(
        data: impl IntoIterator<Item = Option<impl Into<std::borrow::Cow<'a, Self>>>>,
        _extension_wrapper: Option<&str>,
    ) -> re_types::SerializationResult<Box<dyn arrow2::array::Array>>
    where
        Self: Clone + 'a,
    {
        let input = data.into_iter().map(|datum| {
            let datum: Option<::std::borrow::Cow<'a, Self>> = datum.map(Into::into);
            datum.map(|d| d.into_owned())
        });
        let vec: Vec<_> = input.collect();
        let arrow = arrow2_convert::serialize::TryIntoArrow::try_into_arrow(vec.iter())
            .map_err(|err| re_types::SerializationError::ArrowConvertFailure(err.to_string()))?;
        Ok(arrow)
    }
    fn try_from_arrow_opt(
        data: &dyn arrow2::array::Array,
    ) -> re_types::DeserializationResult<Vec<Option<Self>>>
    where
        Self: Sized,
    {
        use arrow2_convert::deserialize::arrow_array_deserialize_iterator;
        let native = arrow_array_deserialize_iterator(data)
            .map_err(|err| re_types::DeserializationError::ArrowConvertFailure(err.to_string()))?
            .collect();
        Ok(native)
    }
    fn try_from_arrow_opt_iter(
        data: &dyn arrow2::array::Array,
    ) -> re_types::DeserializationResult<Self::IterItem<'_>>
    where
        Self: Sized,
    {
        let native =
        <<Self as arrow2_convert::deserialize::ArrowDeserialize>::ArrayType as arrow2_convert::deserialize::ArrowArray>::iter_from_array_ref(data);
        Ok(native)
    }

    fn iter_mapper(item: Self::Item<'_>) -> Option<Self> {
        <Self as arrow2_convert::deserialize::ArrowDeserialize>::arrow_deserialize(item)
    }
}
impl re_types::Component for ColorRGBA {}

impl<'a> From<ColorRGBA> for ::std::borrow::Cow<'a, ColorRGBA> {
    #[inline]
    fn from(value: ColorRGBA) -> Self {
        std::borrow::Cow::Owned(value)
    }
}
impl<'a> From<&'a ColorRGBA> for ::std::borrow::Cow<'a, ColorRGBA> {
    #[inline]
    fn from(value: &'a ColorRGBA) -> Self {
        std::borrow::Cow::Borrowed(value)
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
