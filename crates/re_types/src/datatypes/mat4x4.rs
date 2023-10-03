// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/re_types/definitions/rerun/datatypes/mat4x4.fbs".

#![allow(trivial_numeric_casts)]
#![allow(unused_parens)]
#![allow(clippy::clone_on_copy)]
#![allow(clippy::iter_on_single_items)]
#![allow(clippy::map_flatten)]
#![allow(clippy::match_wildcard_for_single_variants)]
#![allow(clippy::needless_question_mark)]
#![allow(clippy::new_without_default)]
#![allow(clippy::redundant_closure)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::unnecessary_cast)]

/// **Datatype**:  A 4x4 Matrix.
///
/// Matrices in Rerun are stored as flat list of coefficients in column-major order:
/// ```text
///            column 0         column 1         column 2         column 3
///        --------------------------------------------------------------------
/// row 0 | flat_columns[0]  flat_columns[4]  flat_columns[8]  flat_columns[12]
/// row 1 | flat_columns[1]  flat_columns[5]  flat_columns[9]  flat_columns[13]
/// row 2 | flat_columns[2]  flat_columns[6]  flat_columns[10] flat_columns[14]
/// row 3 | flat_columns[3]  flat_columns[7]  flat_columns[11] flat_columns[15]
/// ```
#[derive(Clone, Debug, Copy, PartialEq, PartialOrd)]
pub struct Mat4x4(
    /// Flat list of matrix coefficients in column-major order.
    pub [f32; 16usize],
);

impl From<[f32; 16usize]> for Mat4x4 {
    #[inline]
    fn from(flat_columns: [f32; 16usize]) -> Self {
        Self(flat_columns)
    }
}

impl From<Mat4x4> for [f32; 16usize] {
    #[inline]
    fn from(value: Mat4x4) -> Self {
        value.0
    }
}

impl<'a> From<Mat4x4> for ::std::borrow::Cow<'a, Mat4x4> {
    #[inline]
    fn from(value: Mat4x4) -> Self {
        std::borrow::Cow::Owned(value)
    }
}

impl<'a> From<&'a Mat4x4> for ::std::borrow::Cow<'a, Mat4x4> {
    #[inline]
    fn from(value: &'a Mat4x4) -> Self {
        std::borrow::Cow::Borrowed(value)
    }
}

impl crate::Loggable for Mat4x4 {
    type Name = crate::DatatypeName;

    #[inline]
    fn name() -> Self::Name {
        "rerun.datatypes.Mat4x4".into()
    }

    #[allow(unused_imports, clippy::wildcard_imports)]
    #[inline]
    fn arrow_datatype() -> arrow2::datatypes::DataType {
        use ::arrow2::datatypes::*;
        DataType::FixedSizeList(
            Box::new(Field {
                name: "item".to_owned(),
                data_type: DataType::Float32,
                is_nullable: false,
                metadata: [].into(),
            }),
            16usize,
        )
    }

    #[allow(unused_imports, clippy::wildcard_imports)]
    fn to_arrow_opt<'a>(
        data: impl IntoIterator<Item = Option<impl Into<::std::borrow::Cow<'a, Self>>>>,
    ) -> crate::SerializationResult<Box<dyn ::arrow2::array::Array>>
    where
        Self: Clone + 'a,
    {
        re_tracing::profile_function!();
        use crate::{Loggable as _, ResultExt as _};
        use ::arrow2::{array::*, datatypes::*};
        Ok({
            let (somes, data0): (Vec<_>, Vec<_>) = data
                .into_iter()
                .map(|datum| {
                    let datum: Option<::std::borrow::Cow<'a, Self>> = datum.map(Into::into);
                    let datum = datum.map(|datum| {
                        let Self(data0) = datum.into_owned();
                        data0
                    });
                    (datum.is_some(), datum)
                })
                .unzip();
            let data0_bitmap: Option<::arrow2::bitmap::Bitmap> = {
                let any_nones = somes.iter().any(|some| !*some);
                any_nones.then(|| somes.into())
            };
            {
                use arrow2::{buffer::Buffer, offset::OffsetsBuffer};
                let data0_inner_data: Vec<_> = data0
                    .iter()
                    .flatten()
                    .flatten()
                    .cloned()
                    .map(Some)
                    .collect();
                let data0_inner_bitmap: Option<::arrow2::bitmap::Bitmap> =
                    data0_bitmap.as_ref().map(|bitmap| {
                        bitmap
                            .iter()
                            .map(|i| std::iter::repeat(i).take(16usize))
                            .flatten()
                            .collect::<Vec<_>>()
                            .into()
                    });
                FixedSizeListArray::new(
                    Self::arrow_datatype(),
                    PrimitiveArray::new(
                        DataType::Float32,
                        data0_inner_data
                            .into_iter()
                            .map(|v| v.unwrap_or_default())
                            .collect(),
                        data0_inner_bitmap,
                    )
                    .boxed(),
                    data0_bitmap,
                )
                .boxed()
            }
        })
    }

    #[allow(unused_imports, clippy::wildcard_imports)]
    fn from_arrow_opt(
        arrow_data: &dyn ::arrow2::array::Array,
    ) -> crate::DeserializationResult<Vec<Option<Self>>>
    where
        Self: Sized,
    {
        re_tracing::profile_function!();
        use crate::{Loggable as _, ResultExt as _};
        use ::arrow2::{array::*, buffer::*, datatypes::*};
        Ok({
            let arrow_data = arrow_data
                .as_any()
                .downcast_ref::<::arrow2::array::FixedSizeListArray>()
                .ok_or_else(|| {
                    crate::DeserializationError::datatype_mismatch(
                        DataType::FixedSizeList(
                            Box::new(Field {
                                name: "item".to_owned(),
                                data_type: DataType::Float32,
                                is_nullable: false,
                                metadata: [].into(),
                            }),
                            16usize,
                        ),
                        arrow_data.data_type().clone(),
                    )
                })
                .with_context("rerun.datatypes.Mat4x4#flat_columns")?;
            if arrow_data.is_empty() {
                Vec::new()
            } else {
                let offsets = (0..)
                    .step_by(16usize)
                    .zip((16usize..).step_by(16usize).take(arrow_data.len()));
                let arrow_data_inner = {
                    let arrow_data_inner = &**arrow_data.values();
                    arrow_data_inner
                        .as_any()
                        .downcast_ref::<Float32Array>()
                        .ok_or_else(|| {
                            crate::DeserializationError::datatype_mismatch(
                                DataType::Float32,
                                arrow_data_inner.data_type().clone(),
                            )
                        })
                        .with_context("rerun.datatypes.Mat4x4#flat_columns")?
                        .into_iter()
                        .map(|opt| opt.copied())
                        .collect::<Vec<_>>()
                };
                arrow2::bitmap::utils::ZipValidity::new_with_validity(
                    offsets,
                    arrow_data.validity(),
                )
                .map(|elem| {
                    elem.map(|(start, end)| {
                        debug_assert!(end - start == 16usize);
                        if end as usize > arrow_data_inner.len() {
                            return Err(crate::DeserializationError::offset_slice_oob(
                                (start, end),
                                arrow_data_inner.len(),
                            ));
                        }

                        #[allow(unsafe_code, clippy::undocumented_unsafe_blocks)]
                        let data =
                            unsafe { arrow_data_inner.get_unchecked(start as usize..end as usize) };
                        let data = data.iter().cloned().map(Option::unwrap_or_default);
                        let arr = array_init::from_iter(data).unwrap();
                        Ok(arr)
                    })
                    .transpose()
                })
                .collect::<crate::DeserializationResult<Vec<Option<_>>>>()?
            }
            .into_iter()
        }
        .map(|v| v.ok_or_else(crate::DeserializationError::missing_data))
        .map(|res| res.map(|v| Some(Self(v))))
        .collect::<crate::DeserializationResult<Vec<Option<_>>>>()
        .with_context("rerun.datatypes.Mat4x4#flat_columns")
        .with_context("rerun.datatypes.Mat4x4")?)
    }
}
