//! Example components to be used for tests and docs

use std::sync::Arc;

use re_arrow_util::ArrowArrayDowncastRef as _;
use re_byte_size::SizeBytes;
use re_types_core::{
    Component, ComponentDescriptor, ComponentType, DeserializationError, Loggable,
    SerializedComponentBatch,
};

// ----------------------------------------------------------------------------

#[derive(Debug, Default)]
pub struct MyPoints {
    pub points: Option<SerializedComponentBatch>,
    pub colors: Option<SerializedComponentBatch>,
    pub labels: Option<SerializedComponentBatch>,
}

impl MyPoints {
    pub const NUM_COMPONENTS: usize = 5;
}

impl MyPoints {
    pub fn new(points: impl IntoIterator<Item = impl Into<MyPoint>>) -> Self {
        Self {
            points: re_types_core::try_serialize_field(Self::descriptor_points(), points),
            ..Self::default()
        }
    }

    pub fn descriptor_points() -> ComponentDescriptor {
        ComponentDescriptor {
            archetype: Some("example.MyPoints".into()),
            component: ("example.MyPoints:points".into()),
            component_type: Some(MyPoint::name()),
        }
    }

    pub fn descriptor_colors() -> ComponentDescriptor {
        ComponentDescriptor {
            archetype: Some("example.MyPoints".into()),
            component: ("example.MyPoints:colors".into()),
            component_type: Some(MyColor::name()),
        }
    }

    pub fn descriptor_labels() -> ComponentDescriptor {
        ComponentDescriptor {
            archetype: Some("example.MyPoints".into()),
            component: ("example.MyPoints:labels".into()),
            component_type: Some(MyLabel::name()),
        }
    }

    pub fn update_fields() -> Self {
        Self::default()
    }

    pub fn clear_fields() -> Self {
        Self {
            points: Some(SerializedComponentBatch::new(
                MyPoint::arrow_empty(),
                Self::descriptor_points(),
            )),
            colors: Some(SerializedComponentBatch::new(
                MyColor::arrow_empty(),
                Self::descriptor_colors(),
            )),
            labels: Some(SerializedComponentBatch::new(
                MyLabel::arrow_empty(),
                Self::descriptor_labels(),
            )),
        }
    }

    #[inline]
    pub fn with_labels(mut self, labels: impl IntoIterator<Item = impl Into<MyLabel>>) -> Self {
        self.labels = re_types_core::try_serialize_field(Self::descriptor_labels(), labels);
        self
    }

    #[inline]
    pub fn with_colors(mut self, colors: impl IntoIterator<Item = impl Into<MyColor>>) -> Self {
        self.colors = re_types_core::try_serialize_field(Self::descriptor_colors(), colors);
        self
    }
}

impl re_types_core::Archetype for MyPoints {
    fn name() -> re_types_core::ArchetypeName {
        "example.MyPoints".into()
    }

    fn display_name() -> &'static str {
        "MyPoints"
    }

    fn required_components() -> ::std::borrow::Cow<'static, [re_types_core::ComponentDescriptor]> {
        vec![Self::descriptor_points()].into()
    }

    fn recommended_components() -> std::borrow::Cow<'static, [re_types_core::ComponentDescriptor]> {
        vec![Self::descriptor_colors(), Self::descriptor_labels()].into()
    }
}

impl ::re_types_core::AsComponents for MyPoints {
    #[inline]
    fn as_serialized_batches(&self) -> Vec<SerializedComponentBatch> {
        [
            self.colors.clone(),
            self.labels.clone(),
            self.points.clone(),
        ]
        .into_iter()
        .flatten()
        .collect()
    }
}

// ----------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Default, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct MyPoint {
    pub x: f32,
    pub y: f32,
}

impl MyPoint {
    #[expect(clippy::should_implement_trait)]
    #[inline]
    pub fn from_iter(it: impl IntoIterator<Item = u32>) -> Vec<Self> {
        it.into_iter()
            .map(|i| Self::new(i as f32, i as f32))
            .collect()
    }

    #[inline]
    pub fn partial_descriptor() -> ComponentDescriptor {
        ComponentDescriptor::partial("my_point")
    }
}

impl MyPoint {
    #[inline]
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

    #[inline]
    fn is_pod() -> bool {
        true
    }
}

impl Loggable for MyPoint {
    fn arrow_datatype() -> arrow::datatypes::DataType {
        use arrow::datatypes::DataType::Float32;
        arrow::datatypes::DataType::Struct(arrow::datatypes::Fields::from(vec![
            arrow::datatypes::Field::new("x", Float32, false),
            arrow::datatypes::Field::new("y", Float32, false),
        ]))
    }

    fn to_arrow_opt<'a>(
        data: impl IntoIterator<Item = Option<impl Into<std::borrow::Cow<'a, Self>>>>,
    ) -> re_types_core::SerializationResult<arrow::array::ArrayRef>
    where
        Self: 'a,
    {
        use arrow::datatypes::DataType::Float32;

        let (xs, ys): (Vec<_>, Vec<_>) = data
            .into_iter()
            .map(Option::unwrap)
            .map(Into::into)
            .map(|p| (p.x, p.y))
            .unzip();

        let x_array = Arc::new(arrow::array::Float32Array::from(xs));
        let y_array = Arc::new(arrow::array::Float32Array::from(ys));

        Ok(Arc::new(arrow::array::StructArray::new(
            arrow::datatypes::Fields::from(vec![
                arrow::datatypes::Field::new("x", Float32, false),
                arrow::datatypes::Field::new("y", Float32, false),
            ]),
            vec![x_array, y_array],
            None,
        )))
    }

    fn from_arrow_opt(
        data: &dyn arrow::array::Array,
    ) -> re_types_core::DeserializationResult<Vec<Option<Self>>> {
        let array = data
            .downcast_array_ref::<arrow::array::StructArray>()
            .ok_or_else(DeserializationError::downcast_error::<arrow::array::StructArray>)?;

        let x_array = array.columns()[0].as_ref();
        let y_array = array.columns()[1].as_ref();

        let xs = x_array
            .downcast_array_ref::<arrow::array::Float32Array>()
            .ok_or_else(DeserializationError::downcast_error::<arrow::array::Float32Array>)?;
        let ys = y_array
            .downcast_array_ref::<arrow::array::Float32Array>()
            .ok_or_else(DeserializationError::downcast_error::<arrow::array::Float32Array>)?;

        Ok(xs
            .iter()
            .zip(ys.iter())
            .map(|(x, y)| {
                if let (Some(x), Some(y)) = (x, y) {
                    Some(Self { x, y })
                } else {
                    None
                }
            })
            .collect())
    }
}

impl Component for MyPoint {
    fn name() -> ComponentType {
        "example.MyPoint".into()
    }
}

// ----------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Default, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct MyPoint64 {
    pub x: f64,
    pub y: f64,
}

impl MyPoint64 {
    #[expect(clippy::should_implement_trait)]
    #[inline]
    pub fn from_iter(it: impl IntoIterator<Item = u64>) -> Vec<Self> {
        it.into_iter()
            .map(|i| Self::new(i as f64, i as f64))
            .collect()
    }

    #[inline]
    pub fn partial_descriptor() -> ComponentDescriptor {
        ComponentDescriptor::partial("my_point_64")
    }
}

impl MyPoint64 {
    #[inline]
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

re_types_core::macros::impl_into_cow!(MyPoint64);

impl SizeBytes for MyPoint64 {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        let Self { x: _, y: _ } = self;
        0
    }

    #[inline]
    fn is_pod() -> bool {
        true
    }
}

impl Loggable for MyPoint64 {
    fn arrow_datatype() -> arrow::datatypes::DataType {
        use arrow::datatypes::DataType::Float64;
        arrow::datatypes::DataType::Struct(arrow::datatypes::Fields::from(vec![
            arrow::datatypes::Field::new("x", Float64, false),
            arrow::datatypes::Field::new("y", Float64, false),
        ]))
    }

    fn to_arrow_opt<'a>(
        data: impl IntoIterator<Item = Option<impl Into<std::borrow::Cow<'a, Self>>>>,
    ) -> re_types_core::SerializationResult<arrow::array::ArrayRef>
    where
        Self: 'a,
    {
        use arrow::datatypes::DataType::Float64;

        let (xs, ys): (Vec<_>, Vec<_>) = data
            .into_iter()
            .map(Option::unwrap)
            .map(Into::into)
            .map(|p| (p.x, p.y))
            .unzip();

        let x_array = Arc::new(arrow::array::Float64Array::from(xs));
        let y_array = Arc::new(arrow::array::Float64Array::from(ys));

        Ok(Arc::new(arrow::array::StructArray::new(
            arrow::datatypes::Fields::from(vec![
                arrow::datatypes::Field::new("x", Float64, false),
                arrow::datatypes::Field::new("y", Float64, false),
            ]),
            vec![x_array, y_array],
            None,
        )))
    }

    fn from_arrow_opt(
        data: &dyn arrow::array::Array,
    ) -> re_types_core::DeserializationResult<Vec<Option<Self>>> {
        let array = data
            .downcast_array_ref::<arrow::array::StructArray>()
            .ok_or_else(DeserializationError::downcast_error::<arrow::array::StructArray>)?;

        let x_array = array.columns()[0].as_ref();
        let y_array = array.columns()[1].as_ref();

        let xs = x_array
            .downcast_array_ref::<arrow::array::Float64Array>()
            .ok_or_else(DeserializationError::downcast_error::<arrow::array::Float64Array>)?;
        let ys = y_array
            .downcast_array_ref::<arrow::array::Float64Array>()
            .ok_or_else(DeserializationError::downcast_error::<arrow::array::Float64Array>)?;

        Ok(xs
            .iter()
            .zip(ys.iter())
            .map(|(x, y)| {
                if let (Some(x), Some(y)) = (x, y) {
                    Some(Self { x, y })
                } else {
                    None
                }
            })
            .collect())
    }
}

impl Component for MyPoint64 {
    fn name() -> ComponentType {
        "example.MyPoint64".into()
    }
}

// ----------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, bytemuck::Pod, bytemuck::Zeroable)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[repr(transparent)]
pub struct MyColor(pub u32);

impl MyColor {
    #[expect(clippy::should_implement_trait)]
    #[inline]
    pub fn from_iter(it: impl IntoIterator<Item = u32>) -> Vec<Self> {
        it.into_iter().map(Self).collect()
    }
}

impl MyColor {
    #[inline]
    pub fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Self(u32::from_le_bytes([r, g, b, 255]))
    }
}

impl From<u32> for MyColor {
    #[inline]
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

    #[inline]
    fn is_pod() -> bool {
        true
    }
}

impl Loggable for MyColor {
    fn arrow_datatype() -> arrow::datatypes::DataType {
        arrow::datatypes::DataType::UInt32
    }

    fn to_arrow_opt<'a>(
        data: impl IntoIterator<Item = Option<impl Into<std::borrow::Cow<'a, Self>>>>,
    ) -> re_types_core::SerializationResult<arrow::array::ArrayRef>
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
        data: &dyn arrow::array::Array,
    ) -> re_types_core::DeserializationResult<Vec<Option<Self>>> {
        use re_types_core::datatypes::UInt32;
        Ok(UInt32::from_arrow_opt(data)?
            .into_iter()
            .map(|opt| opt.map(|v| Self(v.0)))
            .collect())
    }
}

impl Component for MyColor {
    fn name() -> ComponentType {
        "example.MyColor".into()
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
    fn arrow_datatype() -> arrow::datatypes::DataType {
        re_types_core::datatypes::Utf8::arrow_datatype()
    }

    fn to_arrow_opt<'a>(
        data: impl IntoIterator<Item = Option<impl Into<std::borrow::Cow<'a, Self>>>>,
    ) -> re_types_core::SerializationResult<arrow::array::ArrayRef>
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
        data: &dyn arrow::array::Array,
    ) -> re_types_core::DeserializationResult<Vec<Option<Self>>> {
        use re_types_core::datatypes::Utf8;
        Ok(Utf8::from_arrow_opt(data)?
            .into_iter()
            .map(|opt| opt.map(|v| Self(v.0.to_string())))
            .collect())
    }
}

impl Component for MyLabel {
    fn name() -> ComponentType {
        "example.MyLabel".into()
    }
}

// ----------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, bytemuck::Pod, bytemuck::Zeroable)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[repr(transparent)]
pub struct MyIndex(pub u64);

impl MyIndex {
    #[expect(clippy::should_implement_trait)]
    #[inline]
    pub fn from_iter(it: impl IntoIterator<Item = u64>) -> Vec<Self> {
        it.into_iter().map(Self).collect()
    }

    #[inline]
    pub fn partial_descriptor() -> ComponentDescriptor {
        ComponentDescriptor {
            component: "my_index".into(),
            archetype: None,
            component_type: Some(Self::name()),
        }
    }
}

re_types_core::macros::impl_into_cow!(MyIndex);

impl SizeBytes for MyIndex {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        let Self(_) = self;
        0
    }

    #[inline]
    fn is_pod() -> bool {
        true
    }
}

impl Loggable for MyIndex {
    fn arrow_datatype() -> arrow::datatypes::DataType {
        arrow::datatypes::DataType::UInt64
    }

    fn to_arrow_opt<'a>(
        data: impl IntoIterator<Item = Option<impl Into<std::borrow::Cow<'a, Self>>>>,
    ) -> re_types_core::SerializationResult<arrow::array::ArrayRef>
    where
        Self: 'a,
    {
        use re_types_core::datatypes::UInt64;
        UInt64::to_arrow_opt(
            data.into_iter()
                .map(|opt| opt.map(Into::into).map(|c| UInt64(c.0))),
        )
    }

    fn from_arrow_opt(
        data: &dyn arrow::array::Array,
    ) -> re_types_core::DeserializationResult<Vec<Option<Self>>> {
        use re_types_core::datatypes::UInt64;
        Ok(UInt64::from_arrow_opt(data)?
            .into_iter()
            .map(|opt| opt.map(|v| Self(v.0)))
            .collect())
    }
}

impl Component for MyIndex {
    fn name() -> re_types_core::ComponentType {
        "example.MyIndex".into()
    }
}
