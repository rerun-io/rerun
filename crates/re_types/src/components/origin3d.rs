// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/re_types/definitions/rerun/components/origin3d.fbs".

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

/// A point of origin in 3D space.
#[derive(Clone, Debug, Copy, PartialEq)]
pub struct Origin3D(pub crate::datatypes::Vec3D);

impl<T: Into<crate::datatypes::Vec3D>> From<T> for Origin3D {
    fn from(v: T) -> Self {
        Self(v.into())
    }
}

impl std::borrow::Borrow<crate::datatypes::Vec3D> for Origin3D {
    #[inline]
    fn borrow(&self) -> &crate::datatypes::Vec3D {
        &self.0
    }
}

impl std::ops::Deref for Origin3D {
    type Target = crate::datatypes::Vec3D;

    #[inline]
    fn deref(&self) -> &crate::datatypes::Vec3D {
        &self.0
    }
}

impl<'a> From<Origin3D> for ::std::borrow::Cow<'a, Origin3D> {
    #[inline]
    fn from(value: Origin3D) -> Self {
        std::borrow::Cow::Owned(value)
    }
}

impl<'a> From<&'a Origin3D> for ::std::borrow::Cow<'a, Origin3D> {
    #[inline]
    fn from(value: &'a Origin3D) -> Self {
        std::borrow::Cow::Borrowed(value)
    }
}

impl crate::Loggable for Origin3D {
    type Name = crate::ComponentName;

    #[inline]
    fn name() -> Self::Name {
        "rerun.components.Origin3D".into()
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
            3usize,
        )
    }

    #[allow(unused_imports, clippy::wildcard_imports)]
    fn to_arrow_opt<'a>(
        data: impl IntoIterator<Item = Option<impl Into<::std::borrow::Cow<'a, Self>>>>,
    ) -> crate::SerializationResult<Box<dyn ::arrow2::array::Array>>
    where
        Self: Clone + 'a,
    {
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
                    .map(|datum| {
                        datum
                            .map(|datum| {
                                let crate::datatypes::Vec3D(data0) = datum;
                                data0
                            })
                            .unwrap_or_default()
                    })
                    .flatten()
                    .map(Some)
                    .collect();
                let data0_inner_bitmap: Option<::arrow2::bitmap::Bitmap> =
                    data0_bitmap.as_ref().map(|bitmap| {
                        bitmap
                            .iter()
                            .map(|i| std::iter::repeat(i).take(3usize))
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
    fn try_from_arrow_opt(
        arrow_data: &dyn ::arrow2::array::Array,
    ) -> crate::DeserializationResult<Vec<Option<Self>>>
    where
        Self: Sized,
    {
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
                            3usize,
                        ),
                        arrow_data.data_type().clone(),
                    )
                })
                .with_context("rerun.components.Origin3D#origin")?;
            if arrow_data.is_empty() {
                Vec::new()
            } else {
                let offsets = (0..)
                    .step_by(3usize)
                    .zip((3usize..).step_by(3usize).take(arrow_data.len()));
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
                        .with_context("rerun.components.Origin3D#origin")?
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
                        debug_assert!(end - start == 3usize);
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
                .map(|res_or_opt| {
                    res_or_opt.map(|res_or_opt| res_or_opt.map(|v| crate::datatypes::Vec3D(v)))
                })
                .collect::<crate::DeserializationResult<Vec<Option<_>>>>()?
            }
            .into_iter()
        }
        .map(|v| v.ok_or_else(crate::DeserializationError::missing_data))
        .map(|res| res.map(|v| Some(Self(v))))
        .collect::<crate::DeserializationResult<Vec<Option<_>>>>()
        .with_context("rerun.components.Origin3D#origin")
        .with_context("rerun.components.Origin3D")?)
    }

    #[allow(unused_imports, clippy::wildcard_imports)]
    #[inline]
    fn try_from_arrow(
        arrow_data: &dyn ::arrow2::array::Array,
    ) -> crate::DeserializationResult<Vec<Self>>
    where
        Self: Sized,
    {
        use crate::{Loggable as _, ResultExt as _};
        use ::arrow2::{array::*, buffer::*, datatypes::*};
        if let Some(validity) = arrow_data.validity() {
            if validity.unset_bits() != 0 {
                return Err(crate::DeserializationError::missing_data());
            }
        }
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
                            3usize,
                        ),
                        arrow_data.data_type().clone(),
                    )
                })
                .with_context("rerun.components.Origin3D#origin")?;
            let arrow_data_inner = &**arrow_data.values();
            bytemuck::cast_slice::<_, [_; 3usize]>(
                arrow_data_inner
                    .as_any()
                    .downcast_ref::<Float32Array>()
                    .ok_or_else(|| {
                        crate::DeserializationError::datatype_mismatch(
                            DataType::Float32,
                            arrow_data_inner.data_type().clone(),
                        )
                    })
                    .with_context("rerun.components.Origin3D#origin")?
                    .values()
                    .as_slice(),
            )
            .iter()
            .copied()
            .map(|v| crate::datatypes::Vec3D(v))
        }
        .map(|v| Self(v))
        .collect::<Vec<_>>())
    }
}
