// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/re_types/definitions/rerun/datatypes/transform3d.fbs".

#![allow(unused_imports)]
#![allow(unused_parens)]
#![allow(clippy::clone_on_copy)]
#![allow(clippy::cloned_instead_of_copied)]
#![allow(clippy::map_flatten)]
#![allow(clippy::needless_question_mark)]
#![allow(clippy::new_without_default)]
#![allow(clippy::redundant_closure)]
#![allow(clippy::too_many_lines)]

use ::re_types_core::external::arrow2;
use ::re_types_core::ComponentName;
use ::re_types_core::SerializationResult;
use ::re_types_core::{ComponentBatch, MaybeOwnedComponentBatch};
use ::re_types_core::{DeserializationError, DeserializationResult};

/// **Datatype**: Representation of a 3D affine transform.
///
/// Rarely used directly, prefer using the underlying representation classes and pass them
/// directly to `Transform3D::child_from_parent` or `Transform3D::parent_from_child`.
#[derive(Clone, Debug, Copy, PartialEq)]
pub enum Transform3D {
    /// Translation plus a 3x3 matrix for scale, rotation, skew, etc.
    TranslationAndMat3x3(crate::datatypes::TranslationAndMat3x3),

    /// Translation, rotation and scale, decomposed.
    TranslationRotationScale(crate::datatypes::TranslationRotationScale3D),
}

impl ::re_types_core::SizeBytes for Transform3D {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        #![allow(clippy::match_same_arms)]
        match self {
            Self::TranslationAndMat3x3(v) => v.heap_size_bytes(),
            Self::TranslationRotationScale(v) => v.heap_size_bytes(),
        }
    }

    #[inline]
    fn is_pod() -> bool {
        <crate::datatypes::TranslationAndMat3x3>::is_pod()
            && <crate::datatypes::TranslationRotationScale3D>::is_pod()
    }
}

::re_types_core::macros::impl_into_cow!(Transform3D);

impl ::re_types_core::Loggable for Transform3D {
    type Name = ::re_types_core::DatatypeName;

    #[inline]
    fn name() -> Self::Name {
        "rerun.datatypes.Transform3D".into()
    }

    #[inline]
    fn arrow_datatype() -> arrow2::datatypes::DataType {
        #![allow(clippy::wildcard_imports)]
        use arrow2::datatypes::*;
        DataType::Union(
            std::sync::Arc::new(vec![
                Field::new("_null_markers", DataType::Null, true),
                Field::new(
                    "TranslationAndMat3x3",
                    <crate::datatypes::TranslationAndMat3x3>::arrow_datatype(),
                    false,
                ),
                Field::new(
                    "TranslationRotationScale",
                    <crate::datatypes::TranslationRotationScale3D>::arrow_datatype(),
                    false,
                ),
            ]),
            Some(std::sync::Arc::new(vec![0i32, 1i32, 2i32])),
            UnionMode::Dense,
        )
    }

    fn to_arrow_opt<'a>(
        data: impl IntoIterator<Item = Option<impl Into<::std::borrow::Cow<'a, Self>>>>,
    ) -> SerializationResult<Box<dyn arrow2::array::Array>>
    where
        Self: Clone + 'a,
    {
        #![allow(clippy::wildcard_imports)]
        use ::re_types_core::{Loggable as _, ResultExt as _};
        use arrow2::{array::*, datatypes::*};
        Ok({
            // Dense Arrow union
            let data: Vec<_> = data
                .into_iter()
                .map(|datum| {
                    let datum: Option<::std::borrow::Cow<'a, Self>> = datum.map(Into::into);
                    datum
                })
                .collect();
            let types = data
                .iter()
                .map(|a| match a.as_deref() {
                    None => 0,
                    Some(Self::TranslationAndMat3x3(_)) => 1i8,
                    Some(Self::TranslationRotationScale(_)) => 2i8,
                })
                .collect();
            let fields = vec![
                NullArray::new(DataType::Null, data.iter().filter(|v| v.is_none()).count()).boxed(),
                {
                    let translation_and_mat3x3: Vec<_> = data
                        .iter()
                        .filter_map(|datum| match datum.as_deref() {
                            Some(Self::TranslationAndMat3x3(v)) => Some(v.clone()),
                            _ => None,
                        })
                        .collect();
                    let translation_and_mat3x3_bitmap: Option<arrow2::bitmap::Bitmap> = None;
                    {
                        _ = translation_and_mat3x3_bitmap;
                        crate::datatypes::TranslationAndMat3x3::to_arrow_opt(
                            translation_and_mat3x3.into_iter().map(Some),
                        )?
                    }
                },
                {
                    let translation_rotation_scale: Vec<_> = data
                        .iter()
                        .filter_map(|datum| match datum.as_deref() {
                            Some(Self::TranslationRotationScale(v)) => Some(v.clone()),
                            _ => None,
                        })
                        .collect();
                    let translation_rotation_scale_bitmap: Option<arrow2::bitmap::Bitmap> = None;
                    {
                        _ = translation_rotation_scale_bitmap;
                        crate::datatypes::TranslationRotationScale3D::to_arrow_opt(
                            translation_rotation_scale.into_iter().map(Some),
                        )?
                    }
                },
            ];
            let offsets = Some({
                let mut translation_and_mat3x3_offset = 0;
                let mut translation_rotation_scale_offset = 0;
                let mut nulls_offset = 0;
                data.iter()
                    .map(|v| match v.as_deref() {
                        None => {
                            let offset = nulls_offset;
                            nulls_offset += 1;
                            offset
                        }
                        Some(Self::TranslationAndMat3x3(_)) => {
                            let offset = translation_and_mat3x3_offset;
                            translation_and_mat3x3_offset += 1;
                            offset
                        }
                        Some(Self::TranslationRotationScale(_)) => {
                            let offset = translation_rotation_scale_offset;
                            translation_rotation_scale_offset += 1;
                            offset
                        }
                    })
                    .collect()
            });
            UnionArray::new(Self::arrow_datatype(), types, fields, offsets).boxed()
        })
    }

    fn from_arrow_opt(
        arrow_data: &dyn arrow2::array::Array,
    ) -> DeserializationResult<Vec<Option<Self>>>
    where
        Self: Sized,
    {
        #![allow(clippy::wildcard_imports)]
        use ::re_types_core::{Loggable as _, ResultExt as _};
        use arrow2::{array::*, buffer::*, datatypes::*};
        Ok({
            let arrow_data = arrow_data
                .as_any()
                .downcast_ref::<arrow2::array::UnionArray>()
                .ok_or_else(|| {
                    let expected = Self::arrow_datatype();
                    let actual = arrow_data.data_type().clone();
                    DeserializationError::datatype_mismatch(expected, actual)
                })
                .with_context("rerun.datatypes.Transform3D")?;
            if arrow_data.is_empty() {
                Vec::new()
            } else {
                let (arrow_data_types, arrow_data_arrays) =
                    (arrow_data.types(), arrow_data.fields());
                let arrow_data_offsets = arrow_data
                    .offsets()
                    .ok_or_else(|| {
                        let expected = Self::arrow_datatype();
                        let actual = arrow_data.data_type().clone();
                        DeserializationError::datatype_mismatch(expected, actual)
                    })
                    .with_context("rerun.datatypes.Transform3D")?;
                if arrow_data_types.len() != arrow_data_offsets.len() {
                    return Err(DeserializationError::offset_slice_oob(
                        (0, arrow_data_types.len()),
                        arrow_data_offsets.len(),
                    ))
                    .with_context("rerun.datatypes.Transform3D");
                }
                let translation_and_mat3x3 = {
                    if 1usize >= arrow_data_arrays.len() {
                        return Ok(Vec::new());
                    }
                    let arrow_data = &*arrow_data_arrays[1usize];
                    crate::datatypes::TranslationAndMat3x3::from_arrow_opt(arrow_data)
                        .with_context("rerun.datatypes.Transform3D#TranslationAndMat3x3")?
                        .into_iter()
                        .collect::<Vec<_>>()
                };
                let translation_rotation_scale = {
                    if 2usize >= arrow_data_arrays.len() {
                        return Ok(Vec::new());
                    }
                    let arrow_data = &*arrow_data_arrays[2usize];
                    crate::datatypes::TranslationRotationScale3D::from_arrow_opt(arrow_data)
                        .with_context("rerun.datatypes.Transform3D#TranslationRotationScale")?
                        .into_iter()
                        .collect::<Vec<_>>()
                };
                arrow_data_types
                    .iter()
                    .enumerate()
                    .map(|(i, typ)| {
                        let offset = arrow_data_offsets[i];
                        if *typ == 0 {
                            Ok(None)
                        } else {
                            Ok(Some(match typ {
                                1i8 => Self::TranslationAndMat3x3({
                                    if offset as usize >= translation_and_mat3x3.len() {
                                        return Err(DeserializationError::offset_oob(
                                            offset as _,
                                            translation_and_mat3x3.len(),
                                        ))
                                        .with_context(
                                            "rerun.datatypes.Transform3D#TranslationAndMat3x3",
                                        );
                                    }

                                    #[allow(unsafe_code, clippy::undocumented_unsafe_blocks)]
                                    unsafe { translation_and_mat3x3.get_unchecked(offset as usize) }
                                        .clone()
                                        .ok_or_else(DeserializationError::missing_data)
                                        .with_context(
                                            "rerun.datatypes.Transform3D#TranslationAndMat3x3",
                                        )?
                                }),
                                2i8 => Self::TranslationRotationScale({
                                    if offset as usize >= translation_rotation_scale.len() {
                                        return Err(DeserializationError::offset_oob(
                                            offset as _,
                                            translation_rotation_scale.len(),
                                        ))
                                        .with_context(
                                            "rerun.datatypes.Transform3D#TranslationRotationScale",
                                        );
                                    }

                                    #[allow(unsafe_code, clippy::undocumented_unsafe_blocks)]
                                    unsafe {
                                        translation_rotation_scale.get_unchecked(offset as usize)
                                    }
                                    .clone()
                                    .ok_or_else(DeserializationError::missing_data)
                                    .with_context(
                                        "rerun.datatypes.Transform3D#TranslationRotationScale",
                                    )?
                                }),
                                _ => {
                                    return Err(DeserializationError::missing_union_arm(
                                        Self::arrow_datatype(),
                                        "<invalid>",
                                        *typ as _,
                                    ));
                                }
                            }))
                        }
                    })
                    .collect::<DeserializationResult<Vec<_>>>()
                    .with_context("rerun.datatypes.Transform3D")?
            }
        })
    }
}
