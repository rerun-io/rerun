// DO NOT EDIT!: This file was auto-generated by crates/re_types_builder/src/codegen/rust/api.rs:165.

#![allow(trivial_numeric_casts)]
#![allow(unused_parens)]
#![allow(clippy::clone_on_copy)]
#![allow(clippy::iter_on_single_items)]
#![allow(clippy::map_flatten)]
#![allow(clippy::match_wildcard_for_single_variants)]
#![allow(clippy::needless_question_mark)]
#![allow(clippy::redundant_closure)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::unnecessary_cast)]

/// An image made up of integer class-ids
///
/// The shape of the `TensorData` must be mappable to an `HxW` tensor.
/// Each pixel corresponds to a depth value in units specified by meter.
///
/// The viewer has limited support for ignoring extra empty dimensions.
///
/// ## Example
///
/// ```ignore
/// //! Create and log a segmentation image.
///
/// use ndarray::{s, Array, ShapeBuilder};
/// use rerun::{
///    archetypes::{AnnotationContext, SegmentationImage},
///    datatypes::Color,
///    RecordingStreamBuilder,
/// };
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///    let (rec, storage) =
///        RecordingStreamBuilder::new("rerun_example_segmentation_image").memory()?;
///
///    // create a segmentation image
///    let mut image = Array::<u8, _>::zeros((200, 300).f());
///    image.slice_mut(s![50..150, 50..120]).fill(1);
///    image.slice_mut(s![100..180, 130..280]).fill(2);
///
///    // create an annotation context to describe the classes
///    let annotation = AnnotationContext::new([
///        (1, "red", Color::from(0xFF0000FF)),
///        (2, "green", Color::from(0x00FF00FF)),
///    ]);
///
///    // log the annotation and the image
///    rec.log("/", &annotation)?;
///
///    rec.log("image", &SegmentationImage::try_from(image)?)?;
///
///    rerun::native_viewer::show(storage.take())?;
///    Ok(())
/// }
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct SegmentationImage {
    /// The image data. Should always be a rank-2 tensor.
    pub data: crate::components::TensorData,

    /// An optional floating point value that specifies the 2D drawing order.
    /// Objects with higher values are drawn on top of those with lower values.
    ///
    /// The default for 2D points is -10.0.
    pub draw_order: Option<crate::components::DrawOrder>,
}

static REQUIRED_COMPONENTS: once_cell::sync::Lazy<[crate::ComponentName; 1usize]> =
    once_cell::sync::Lazy::new(|| ["rerun.components.TensorData".into()]);

static RECOMMENDED_COMPONENTS: once_cell::sync::Lazy<[crate::ComponentName; 0usize]> =
    once_cell::sync::Lazy::new(|| []);

static OPTIONAL_COMPONENTS: once_cell::sync::Lazy<[crate::ComponentName; 1usize]> =
    once_cell::sync::Lazy::new(|| ["rerun.draw_order".into()]);

static ALL_COMPONENTS: once_cell::sync::Lazy<[crate::ComponentName; 2usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            "rerun.components.TensorData".into(),
            "rerun.draw_order".into(),
        ]
    });

impl SegmentationImage {
    pub const NUM_COMPONENTS: usize = 2usize;
}

impl crate::Archetype for SegmentationImage {
    #[inline]
    fn name() -> crate::ArchetypeName {
        "rerun.archetypes.SegmentationImage".into()
    }

    #[inline]
    fn required_components() -> ::std::borrow::Cow<'static, [crate::ComponentName]> {
        REQUIRED_COMPONENTS.as_slice().into()
    }

    #[inline]
    fn recommended_components() -> ::std::borrow::Cow<'static, [crate::ComponentName]> {
        RECOMMENDED_COMPONENTS.as_slice().into()
    }

    #[inline]
    fn optional_components() -> ::std::borrow::Cow<'static, [crate::ComponentName]> {
        OPTIONAL_COMPONENTS.as_slice().into()
    }

    #[inline]
    fn all_components() -> ::std::borrow::Cow<'static, [crate::ComponentName]> {
        ALL_COMPONENTS.as_slice().into()
    }

    #[inline]
    fn indicator_component() -> crate::ComponentName {
        "rerun.components.SegmentationImageIndicator".into()
    }

    #[inline]
    fn num_instances(&self) -> usize {
        1
    }

    fn as_component_lists(&self) -> Vec<&dyn crate::ComponentList> {
        [
            Some(&self.data as &dyn crate::ComponentList),
            self.draw_order
                .as_ref()
                .map(|comp| comp as &dyn crate::ComponentList),
        ]
        .into_iter()
        .flatten()
        .collect()
    }

    #[inline]
    fn try_to_arrow(
        &self,
    ) -> crate::SerializationResult<
        Vec<(::arrow2::datatypes::Field, Box<dyn ::arrow2::array::Array>)>,
    > {
        use crate::{Loggable as _, ResultExt as _};
        Ok([
            {
                Some({
                    let array = <crate::components::TensorData>::try_to_arrow([&self.data]);
                    array.map(|array| {
                        let datatype = ::arrow2::datatypes::DataType::Extension(
                            "rerun.components.TensorData".into(),
                            Box::new(array.data_type().clone()),
                            Some("rerun.components.TensorData".into()),
                        );
                        (
                            ::arrow2::datatypes::Field::new("data", datatype, false),
                            array,
                        )
                    })
                })
                .transpose()
                .with_context("rerun.archetypes.SegmentationImage#data")?
            },
            {
                self.draw_order
                    .as_ref()
                    .map(|single| {
                        let array = <crate::components::DrawOrder>::try_to_arrow([single]);
                        array.map(|array| {
                            let datatype = ::arrow2::datatypes::DataType::Extension(
                                "rerun.components.DrawOrder".into(),
                                Box::new(array.data_type().clone()),
                                Some("rerun.draw_order".into()),
                            );
                            (
                                ::arrow2::datatypes::Field::new("draw_order", datatype, false),
                                array,
                            )
                        })
                    })
                    .transpose()
                    .with_context("rerun.archetypes.SegmentationImage#draw_order")?
            },
            {
                let datatype = ::arrow2::datatypes::DataType::Extension(
                    "rerun.components.SegmentationImageIndicator".to_owned(),
                    Box::new(::arrow2::datatypes::DataType::Null),
                    Some("rerun.components.SegmentationImageIndicator".to_owned()),
                );
                let array = ::arrow2::array::NullArray::new(
                    datatype.to_logical_type().clone(),
                    self.num_instances(),
                )
                .boxed();
                Some((
                    ::arrow2::datatypes::Field::new(
                        "rerun.components.SegmentationImageIndicator",
                        datatype,
                        false,
                    ),
                    array,
                ))
            },
        ]
        .into_iter()
        .flatten()
        .collect())
    }

    #[inline]
    fn try_from_arrow(
        arrow_data: impl IntoIterator<
            Item = (::arrow2::datatypes::Field, Box<dyn ::arrow2::array::Array>),
        >,
    ) -> crate::DeserializationResult<Self> {
        use crate::{Loggable as _, ResultExt as _};
        let arrays_by_name: ::std::collections::HashMap<_, _> = arrow_data
            .into_iter()
            .map(|(field, array)| (field.name, array))
            .collect();
        let data = {
            let array = arrays_by_name
                .get("data")
                .ok_or_else(crate::DeserializationError::missing_data)
                .with_context("rerun.archetypes.SegmentationImage#data")?;
            <crate::components::TensorData>::try_from_arrow_opt(&**array)
                .with_context("rerun.archetypes.SegmentationImage#data")?
                .into_iter()
                .next()
                .flatten()
                .ok_or_else(crate::DeserializationError::missing_data)
                .with_context("rerun.archetypes.SegmentationImage#data")?
        };
        let draw_order = if let Some(array) = arrays_by_name.get("draw_order") {
            Some({
                <crate::components::DrawOrder>::try_from_arrow_opt(&**array)
                    .with_context("rerun.archetypes.SegmentationImage#draw_order")?
                    .into_iter()
                    .next()
                    .flatten()
                    .ok_or_else(crate::DeserializationError::missing_data)
                    .with_context("rerun.archetypes.SegmentationImage#draw_order")?
            })
        } else {
            None
        };
        Ok(Self { data, draw_order })
    }
}

impl SegmentationImage {
    pub fn new(data: impl Into<crate::components::TensorData>) -> Self {
        Self {
            data: data.into(),
            draw_order: None,
        }
    }

    pub fn with_draw_order(mut self, draw_order: impl Into<crate::components::DrawOrder>) -> Self {
        self.draw_order = Some(draw_order.into());
        self
    }
}
