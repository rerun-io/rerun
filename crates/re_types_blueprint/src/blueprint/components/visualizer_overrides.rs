// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/re_types/definitions/rerun/blueprint/components/visualizer_overrides.fbs".

#![allow(trivial_numeric_casts)]
#![allow(unused_imports)]
#![allow(unused_parens)]
#![allow(clippy::clone_on_copy)]
#![allow(clippy::cloned_instead_of_copied)]
#![allow(clippy::iter_on_single_items)]
#![allow(clippy::map_flatten)]
#![allow(clippy::match_wildcard_for_single_variants)]
#![allow(clippy::needless_question_mark)]
#![allow(clippy::new_without_default)]
#![allow(clippy::redundant_closure)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::unnecessary_cast)]

use ::re_types_core::external::arrow2;
use ::re_types_core::ComponentName;
use ::re_types_core::SerializationResult;
use ::re_types_core::{ComponentBatch, MaybeOwnedComponentBatch};
use ::re_types_core::{DeserializationError, DeserializationResult};

/// **Component**: Override the visualizers for an entity.
///
/// This component is a stop-gap mechanism based on the current implementation details
/// of the visualizer system. It is not intended to be a long-term solution, but provides
/// enough utility to be useful in the short term.
///
/// The long-term solution is likely to be based off: <https://github.com/rerun-io/rerun/issues/6626>
///
/// This can only be used as part of blueprints. It will have no effect if used
/// in a regular entity.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Default)]
#[repr(transparent)]
pub struct VisualizerOverrides(
    /// Names of the visualizers that should be active.
    ///
    /// The built-in visualizers are:
    /// - BarChartView
    /// - Arrows2D
    /// - Arrows3D
    /// - Asset3D
    /// - Boxes2D
    /// - Boxes3D
    /// - Cameras
    /// - Images
    /// - Lines2D
    /// - Lines3D
    /// - Mesh3D
    /// - Points2D
    /// - Points3D
    /// - Transform3DArrows
    /// - Tensor
    /// - TextDocument
    /// - TextLog
    /// - SeriesLine
    /// - SeriesPoint
    pub Vec<::re_types_core::ArrowString>,
);

impl ::re_types_core::SizeBytes for VisualizerOverrides {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.0.heap_size_bytes()
    }

    #[inline]
    fn is_pod() -> bool {
        <Vec<::re_types_core::ArrowString>>::is_pod()
    }
}

impl From<Vec<::re_types_core::ArrowString>> for VisualizerOverrides {
    #[inline]
    fn from(visualizers: Vec<::re_types_core::ArrowString>) -> Self {
        Self(visualizers)
    }
}

impl From<VisualizerOverrides> for Vec<::re_types_core::ArrowString> {
    #[inline]
    fn from(value: VisualizerOverrides) -> Self {
        value.0
    }
}

impl std::ops::Deref for VisualizerOverrides {
    type Target = Vec<::re_types_core::ArrowString>;

    #[inline]
    fn deref(&self) -> &Vec<::re_types_core::ArrowString> {
        &self.0
    }
}

impl std::ops::DerefMut for VisualizerOverrides {
    #[inline]
    fn deref_mut(&mut self) -> &mut Vec<::re_types_core::ArrowString> {
        &mut self.0
    }
}

::re_types_core::macros::impl_into_cow!(VisualizerOverrides);

impl ::re_types_core::Loggable for VisualizerOverrides {
    type Name = ::re_types_core::ComponentName;

    #[inline]
    fn name() -> Self::Name {
        "rerun.blueprint.components.VisualizerOverrides".into()
    }

    #[allow(clippy::wildcard_imports)]
    #[inline]
    fn arrow_datatype() -> arrow2::datatypes::DataType {
        use arrow2::datatypes::*;
        DataType::List(std::sync::Arc::new(Field::new(
            "item",
            DataType::Utf8,
            false,
        )))
    }

    #[allow(clippy::wildcard_imports)]
    fn to_arrow_opt<'a>(
        data: impl IntoIterator<Item = Option<impl Into<::std::borrow::Cow<'a, Self>>>>,
    ) -> SerializationResult<Box<dyn arrow2::array::Array>>
    where
        Self: Clone + 'a,
    {
        use ::re_types_core::{Loggable as _, ResultExt as _};
        use arrow2::{array::*, datatypes::*};
        Ok({
            let (somes, data0): (Vec<_>, Vec<_>) = data
                .into_iter()
                .map(|datum| {
                    let datum: Option<::std::borrow::Cow<'a, Self>> = datum.map(Into::into);
                    let datum = datum.map(|datum| datum.into_owned().0);
                    (datum.is_some(), datum)
                })
                .unzip();
            let data0_bitmap: Option<arrow2::bitmap::Bitmap> = {
                let any_nones = somes.iter().any(|some| !*some);
                any_nones.then(|| somes.into())
            };
            {
                use arrow2::{buffer::Buffer, offset::OffsetsBuffer};
                let offsets = arrow2::offset::Offsets::<i32>::try_from_lengths(
                    data0
                        .iter()
                        .map(|opt| opt.as_ref().map_or(0, |datum| datum.len())),
                )?
                .into();
                let data0_inner_data: Vec<_> = data0.into_iter().flatten().flatten().collect();
                let data0_inner_bitmap: Option<arrow2::bitmap::Bitmap> = None;
                ListArray::try_new(
                    Self::arrow_datatype(),
                    offsets,
                    {
                        let offsets = arrow2::offset::Offsets::<i32>::try_from_lengths(
                            data0_inner_data.iter().map(|datum| datum.len()),
                        )?
                        .into();
                        let inner_data: arrow2::buffer::Buffer<u8> =
                            data0_inner_data.into_iter().flat_map(|s| s.0).collect();

                        #[allow(unsafe_code, clippy::undocumented_unsafe_blocks)]
                        unsafe {
                            Utf8Array::<i32>::new_unchecked(
                                DataType::Utf8,
                                offsets,
                                inner_data,
                                data0_inner_bitmap,
                            )
                        }
                        .boxed()
                    },
                    data0_bitmap,
                )?
                .boxed()
            }
        })
    }

    #[allow(clippy::wildcard_imports)]
    fn from_arrow_opt(
        arrow_data: &dyn arrow2::array::Array,
    ) -> DeserializationResult<Vec<Option<Self>>>
    where
        Self: Sized,
    {
        use ::re_types_core::{Loggable as _, ResultExt as _};
        use arrow2::{array::*, buffer::*, datatypes::*};
        Ok({
            let arrow_data = arrow_data
                .as_any()
                .downcast_ref::<arrow2::array::ListArray<i32>>()
                .ok_or_else(|| {
                    let expected = Self::arrow_datatype();
                    let actual = arrow_data.data_type().clone();
                    DeserializationError::datatype_mismatch(expected, actual)
                })
                .with_context("rerun.blueprint.components.VisualizerOverrides#visualizers")?;
            if arrow_data.is_empty() {
                Vec::new()
            } else {
                let arrow_data_inner = {
                    let arrow_data_inner = &**arrow_data.values();
                    {
                        let arrow_data_inner = arrow_data_inner
                            .as_any()
                            .downcast_ref::<arrow2::array::Utf8Array<i32>>()
                            .ok_or_else(|| {
                                let expected = DataType::Utf8;
                                let actual = arrow_data_inner.data_type().clone();
                                DeserializationError::datatype_mismatch(expected, actual)
                            })
                            .with_context(
                                "rerun.blueprint.components.VisualizerOverrides#visualizers",
                            )?;
                        let arrow_data_inner_buf = arrow_data_inner.values();
                        let offsets = arrow_data_inner.offsets();
                        arrow2::bitmap::utils::ZipValidity::new_with_validity(
                            offsets.iter().zip(offsets.lengths()),
                            arrow_data_inner.validity(),
                        )
                        .map(|elem| {
                            elem.map(|(start, len)| {
                                let start = *start as usize;
                                let end = start + len;
                                if end as usize > arrow_data_inner_buf.len() {
                                    return Err(DeserializationError::offset_slice_oob(
                                        (start, end),
                                        arrow_data_inner_buf.len(),
                                    ));
                                }

                                #[allow(unsafe_code, clippy::undocumented_unsafe_blocks)]
                                let data = unsafe {
                                    arrow_data_inner_buf.clone().sliced_unchecked(start, len)
                                };
                                Ok(data)
                            })
                            .transpose()
                        })
                        .map(|res_or_opt| {
                            res_or_opt.map(|res_or_opt| {
                                res_or_opt.map(|v| ::re_types_core::ArrowString(v))
                            })
                        })
                        .collect::<DeserializationResult<Vec<Option<_>>>>()
                        .with_context("rerun.blueprint.components.VisualizerOverrides#visualizers")?
                        .into_iter()
                    }
                    .collect::<Vec<_>>()
                };
                let offsets = arrow_data.offsets();
                arrow2::bitmap::utils::ZipValidity::new_with_validity(
                    offsets.iter().zip(offsets.lengths()),
                    arrow_data.validity(),
                )
                .map(|elem| {
                    elem.map(|(start, len)| {
                        let start = *start as usize;
                        let end = start + len;
                        if end as usize > arrow_data_inner.len() {
                            return Err(DeserializationError::offset_slice_oob(
                                (start, end),
                                arrow_data_inner.len(),
                            ));
                        }

                        #[allow(unsafe_code, clippy::undocumented_unsafe_blocks)]
                        let data =
                            unsafe { arrow_data_inner.get_unchecked(start as usize..end as usize) };
                        let data = data
                            .iter()
                            .cloned()
                            .map(Option::unwrap_or_default)
                            .collect();
                        Ok(data)
                    })
                    .transpose()
                })
                .collect::<DeserializationResult<Vec<Option<_>>>>()?
            }
            .into_iter()
        }
        .map(|v| v.ok_or_else(DeserializationError::missing_data))
        .map(|res| res.map(|v| Some(Self(v))))
        .collect::<DeserializationResult<Vec<Option<_>>>>()
        .with_context("rerun.blueprint.components.VisualizerOverrides#visualizers")
        .with_context("rerun.blueprint.components.VisualizerOverrides")?)
    }
}
