//! Example components to be used for tests and docs

use re_types_core::{Loggable, SizeBytes};

// ----------------------------------------------------------------------------

#[derive(Debug)]
pub struct MyPoints;

impl re_types_core::Archetype for MyPoints {
    type Indicator = re_types_core::GenericIndicatorComponent<Self>;

    fn name() -> re_types_core::ArchetypeName {
        "test.MyPoints".into()
    }

    fn required_components() -> ::std::borrow::Cow<'static, [re_types_core::ComponentName]> {
        vec![MyPoint::name()].into()
    }

    fn recommended_components() -> std::borrow::Cow<'static, [re_types_core::ComponentName]> {
        vec![MyColor::name(), MyLabel::name()].into()
    }
}

// ----------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MyPoint {
    pub x: f32,
    pub y: f32,
}

impl MyPoint {
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

re_types_core::macros::impl_into_cow!(MyPoint);

impl SizeBytes for MyPoint {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        let Self { x: _, y: _ } = self;
        0
    }
}

impl Loggable for MyPoint {
    type Name = re_types_core::ComponentName;

    fn name() -> Self::Name {
        "example.MyPoint".into()
    }

    fn arrow_datatype() -> arrow2::datatypes::DataType {
        use arrow2::datatypes::DataType::Float32;
        arrow2::datatypes::DataType::Struct(vec![
            arrow2::datatypes::Field::new("x", Float32, false),
            arrow2::datatypes::Field::new("y", Float32, false),
        ])
    }

    fn to_arrow_opt<'a>(
        data: impl IntoIterator<Item = Option<impl Into<std::borrow::Cow<'a, Self>>>>,
    ) -> re_types_core::SerializationResult<Box<dyn arrow2::array::Array>>
    where
        Self: 'a,
    {
        let (xs, ys): (Vec<_>, Vec<_>) = data
            .into_iter()
            .map(Option::unwrap)
            .map(Into::into)
            .map(|p| (p.x, p.y))
            .unzip();

        let x_array = arrow2::array::Float32Array::from_vec(xs).boxed();
        let y_array = arrow2::array::Float32Array::from_vec(ys).boxed();

        Ok(
            arrow2::array::StructArray::new(Self::arrow_datatype(), vec![x_array, y_array], None)
                .boxed(),
        )
    }

    fn from_arrow_opt(
        data: &dyn arrow2::array::Array,
    ) -> re_types_core::DeserializationResult<Vec<Option<Self>>> {
        let array = data
            .as_any()
            .downcast_ref::<arrow2::array::StructArray>()
            .unwrap();

        let x_array = array.values()[0].as_ref();
        let y_array = array.values()[1].as_ref();

        let xs = x_array
            .as_any()
            .downcast_ref::<arrow2::array::Float32Array>()
            .unwrap();
        let ys = y_array
            .as_any()
            .downcast_ref::<arrow2::array::Float32Array>()
            .unwrap();

        Ok(xs
            .values_iter()
            .copied()
            .zip(ys.values_iter().copied())
            .map(|(x, y)| Self { x, y })
            .map(Some)
            .collect())
    }
}

// ----------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[repr(transparent)]
pub struct MyColor(pub u32);

impl From<u32> for MyColor {
    fn from(value: u32) -> Self {
        Self(value)
    }
}

re_types_core::macros::impl_into_cow!(MyColor);

impl SizeBytes for MyColor {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        let Self(_) = self;
        0
    }
}

impl Loggable for MyColor {
    type Name = re_types_core::ComponentName;

    fn name() -> Self::Name {
        "example.MyColor".into()
    }

    fn arrow_datatype() -> arrow2::datatypes::DataType {
        arrow2::datatypes::DataType::UInt32
    }

    fn to_arrow_opt<'a>(
        data: impl IntoIterator<Item = Option<impl Into<std::borrow::Cow<'a, Self>>>>,
    ) -> re_types_core::SerializationResult<Box<dyn arrow2::array::Array>>
    where
        Self: 'a,
    {
        use re_types_core::datatypes::UInt32;
        UInt32::to_arrow_opt(
            data.into_iter()
                .map(|opt| opt.map(Into::into).map(|c| UInt32(c.0))),
        )
    }

    fn from_arrow_opt(
        data: &dyn arrow2::array::Array,
    ) -> re_types_core::DeserializationResult<Vec<Option<Self>>> {
        use re_types_core::datatypes::UInt32;
        Ok(UInt32::from_arrow_opt(data)?
            .into_iter()
            .map(|opt| opt.map(|v| Self(v.0)))
            .collect())
    }
}

// ----------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct MyLabel(pub String);

re_types_core::macros::impl_into_cow!(MyLabel);

impl SizeBytes for MyLabel {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        let Self(s) = self;
        s.heap_size_bytes()
    }
}

impl Loggable for MyLabel {
    type Name = re_types_core::ComponentName;

    fn name() -> Self::Name {
        "example.MyLabel".into()
    }

    fn arrow_datatype() -> arrow2::datatypes::DataType {
        re_types_core::datatypes::Utf8::arrow_datatype()
    }

    fn to_arrow_opt<'a>(
        data: impl IntoIterator<Item = Option<impl Into<std::borrow::Cow<'a, Self>>>>,
    ) -> re_types_core::SerializationResult<Box<dyn arrow2::array::Array>>
    where
        Self: 'a,
    {
        use re_types_core::datatypes::Utf8;
        Utf8::to_arrow_opt(
            data.into_iter()
                .map(|opt| opt.map(Into::into).map(|l| Utf8(l.0.clone().into()))),
        )
    }

    fn from_arrow_opt(
        data: &dyn arrow2::array::Array,
    ) -> re_types_core::DeserializationResult<Vec<Option<Self>>> {
        use re_types_core::datatypes::Utf8;
        Ok(Utf8::from_arrow_opt(data)?
            .into_iter()
            .map(|opt| opt.map(|v| Self(v.0.to_string())))
            .collect())
    }
}
