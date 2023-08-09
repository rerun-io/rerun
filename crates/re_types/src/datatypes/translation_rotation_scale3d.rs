// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.

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

/// Representation of an affine transform via separate translation, rotation & scale.
#[derive(Clone, Debug, Copy, PartialEq)]
pub struct TranslationRotationScale3D {
    /// 3D translation vector, applied last.
    pub translation: Option<crate::datatypes::Vec3D>,

    /// 3D rotation, applied second.
    pub rotation: Option<crate::datatypes::Rotation3D>,

    /// 3D scale, applied first.
    pub scale: Option<crate::datatypes::Scale3D>,

    /// If true, the transform maps from the parent space to the space where the transform was logged.
    /// Otherwise, the transform maps from the space to its parent.
    pub from_parent: bool,
}

impl<'a> From<TranslationRotationScale3D> for ::std::borrow::Cow<'a, TranslationRotationScale3D> {
    #[inline]
    fn from(value: TranslationRotationScale3D) -> Self {
        std::borrow::Cow::Owned(value)
    }
}

impl<'a> From<&'a TranslationRotationScale3D>
    for ::std::borrow::Cow<'a, TranslationRotationScale3D>
{
    #[inline]
    fn from(value: &'a TranslationRotationScale3D) -> Self {
        std::borrow::Cow::Borrowed(value)
    }
}

impl crate::Loggable for TranslationRotationScale3D {
    type Name = crate::DatatypeName;
    type Item<'a> = Option<Self>;
    type Iter<'a> = <Vec<Self::Item<'a>> as IntoIterator>::IntoIter;

    #[inline]
    fn name() -> Self::Name {
        "rerun.datatypes.TranslationRotationScale3D".into()
    }

    #[allow(unused_imports, clippy::wildcard_imports)]
    #[inline]
    fn to_arrow_datatype() -> arrow2::datatypes::DataType {
        use ::arrow2::datatypes::*;
        DataType::Struct(vec![
            Field {
                name: "translation".to_owned(),
                data_type: <crate::datatypes::Vec3D>::to_arrow_datatype(),
                is_nullable: true,
                metadata: [].into(),
            },
            Field {
                name: "rotation".to_owned(),
                data_type: <crate::datatypes::Rotation3D>::to_arrow_datatype(),
                is_nullable: true,
                metadata: [].into(),
            },
            Field {
                name: "scale".to_owned(),
                data_type: <crate::datatypes::Scale3D>::to_arrow_datatype(),
                is_nullable: true,
                metadata: [].into(),
            },
            Field {
                name: "from_parent".to_owned(),
                data_type: DataType::Boolean,
                is_nullable: false,
                metadata: [].into(),
            },
        ])
    }

    #[allow(unused_imports, clippy::wildcard_imports)]
    fn try_to_arrow_opt<'a>(
        data: impl IntoIterator<Item = Option<impl Into<::std::borrow::Cow<'a, Self>>>>,
        extension_wrapper: Option<&str>,
    ) -> crate::SerializationResult<Box<dyn ::arrow2::array::Array>>
    where
        Self: Clone + 'a,
    {
        use crate::{Loggable as _, ResultExt as _};
        use ::arrow2::{array::*, datatypes::*};
        Ok({
            let (somes, data): (Vec<_>, Vec<_>) = data
                .into_iter()
                .map(|datum| {
                    let datum: Option<::std::borrow::Cow<'a, Self>> = datum.map(Into::into);
                    (datum.is_some(), datum)
                })
                .unzip();
            let bitmap: Option<::arrow2::bitmap::Bitmap> = {
                let any_nones = somes.iter().any(|some| !*some);
                any_nones.then(|| somes.into())
            };
            StructArray::new(
                (if let Some(ext) = extension_wrapper {
                    DataType::Extension(
                        ext.to_owned(),
                        Box::new(
                            <crate::datatypes::TranslationRotationScale3D>::to_arrow_datatype(),
                        ),
                        None,
                    )
                } else {
                    <crate::datatypes::TranslationRotationScale3D>::to_arrow_datatype()
                })
                .to_logical_type()
                .clone(),
                vec![
                    {
                        let (somes, translation): (Vec<_>, Vec<_>) = data
                            .iter()
                            .map(|datum| {
                                let datum = datum
                                    .as_ref()
                                    .map(|datum| {
                                        let Self { translation, .. } = &**datum;
                                        translation.clone()
                                    })
                                    .flatten();
                                (datum.is_some(), datum)
                            })
                            .unzip();
                        let translation_bitmap: Option<::arrow2::bitmap::Bitmap> = {
                            let any_nones = somes.iter().any(|some| !*some);
                            any_nones.then(|| somes.into())
                        };
                        {
                            use arrow2::{buffer::Buffer, offset::OffsetsBuffer};
                            let translation_inner_data: Vec<_> = translation
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
                            let translation_inner_bitmap: Option<::arrow2::bitmap::Bitmap> = None;
                            FixedSizeListArray::new(
                                {
                                    _ = extension_wrapper;
                                    DataType::FixedSizeList(
                                        Box::new(Field {
                                            name: "item".to_owned(),
                                            data_type: DataType::Float32,
                                            is_nullable: false,
                                            metadata: [].into(),
                                        }),
                                        3usize,
                                    )
                                    .to_logical_type()
                                    .clone()
                                },
                                PrimitiveArray::new(
                                    {
                                        _ = extension_wrapper;
                                        DataType::Float32.to_logical_type().clone()
                                    },
                                    translation_inner_data
                                        .into_iter()
                                        .map(|v| v.unwrap_or_default())
                                        .collect(),
                                    translation_inner_bitmap,
                                )
                                .boxed(),
                                translation_bitmap,
                            )
                            .boxed()
                        }
                    },
                    {
                        let (somes, rotation): (Vec<_>, Vec<_>) = data
                            .iter()
                            .map(|datum| {
                                let datum = datum
                                    .as_ref()
                                    .map(|datum| {
                                        let Self { rotation, .. } = &**datum;
                                        rotation.clone()
                                    })
                                    .flatten();
                                (datum.is_some(), datum)
                            })
                            .unzip();
                        let rotation_bitmap: Option<::arrow2::bitmap::Bitmap> = {
                            let any_nones = somes.iter().any(|some| !*some);
                            any_nones.then(|| somes.into())
                        };
                        {
                            _ = rotation_bitmap;
                            _ = extension_wrapper;
                            crate::datatypes::Rotation3D::try_to_arrow_opt(rotation, None::<&str>)?
                        }
                    },
                    {
                        let (somes, scale): (Vec<_>, Vec<_>) = data
                            .iter()
                            .map(|datum| {
                                let datum = datum
                                    .as_ref()
                                    .map(|datum| {
                                        let Self { scale, .. } = &**datum;
                                        scale.clone()
                                    })
                                    .flatten();
                                (datum.is_some(), datum)
                            })
                            .unzip();
                        let scale_bitmap: Option<::arrow2::bitmap::Bitmap> = {
                            let any_nones = somes.iter().any(|some| !*some);
                            any_nones.then(|| somes.into())
                        };
                        {
                            _ = scale_bitmap;
                            _ = extension_wrapper;
                            crate::datatypes::Scale3D::try_to_arrow_opt(scale, None::<&str>)?
                        }
                    },
                    {
                        let (somes, from_parent): (Vec<_>, Vec<_>) = data
                            .iter()
                            .map(|datum| {
                                let datum = datum.as_ref().map(|datum| {
                                    let Self { from_parent, .. } = &**datum;
                                    from_parent.clone()
                                });
                                (datum.is_some(), datum)
                            })
                            .unzip();
                        let from_parent_bitmap: Option<::arrow2::bitmap::Bitmap> = {
                            let any_nones = somes.iter().any(|some| !*some);
                            any_nones.then(|| somes.into())
                        };
                        BooleanArray::new(
                            {
                                _ = extension_wrapper;
                                DataType::Boolean.to_logical_type().clone()
                            },
                            from_parent
                                .into_iter()
                                .map(|v| v.unwrap_or_default())
                                .collect(),
                            from_parent_bitmap,
                        )
                        .boxed()
                    },
                ],
                bitmap,
            )
            .boxed()
        })
    }

    #[allow(unused_imports, clippy::wildcard_imports)]
    fn try_from_arrow_opt(
        data: &dyn ::arrow2::array::Array,
    ) -> crate::DeserializationResult<Vec<Option<Self>>>
    where
        Self: Sized,
    {
        use crate::{Loggable as _, ResultExt as _};
        use ::arrow2::{array::*, buffer::*, datatypes::*};
        Ok({
            let data = data
                .as_any()
                .downcast_ref::<::arrow2::array::StructArray>()
                .ok_or_else(|| {
                    crate::DeserializationError::datatype_mismatch(
                        DataType::Struct(vec![
                            Field {
                                name: "translation".to_owned(),
                                data_type: <crate::datatypes::Vec3D>::to_arrow_datatype(),
                                is_nullable: true,
                                metadata: [].into(),
                            },
                            Field {
                                name: "rotation".to_owned(),
                                data_type: <crate::datatypes::Rotation3D>::to_arrow_datatype(),
                                is_nullable: true,
                                metadata: [].into(),
                            },
                            Field {
                                name: "scale".to_owned(),
                                data_type: <crate::datatypes::Scale3D>::to_arrow_datatype(),
                                is_nullable: true,
                                metadata: [].into(),
                            },
                            Field {
                                name: "from_parent".to_owned(),
                                data_type: DataType::Boolean,
                                is_nullable: false,
                                metadata: [].into(),
                            },
                        ]),
                        data.data_type().clone(),
                    )
                })
                .with_context("rerun.datatypes.TranslationRotationScale3D")?;
            if data.is_empty() {
                Vec::new()
            } else {
                let (data_fields, data_arrays) = (data.fields(), data.values());
                let arrays_by_name: ::std::collections::HashMap<_, _> = data_fields
                    .iter()
                    .map(|field| field.name.as_str())
                    .zip(data_arrays)
                    .collect();
                let translation = {
                    if !arrays_by_name.contains_key("translation") {
                        return Err(crate::DeserializationError::missing_struct_field(
                            DataType::Struct(vec![
                                Field {
                                    name: "translation".to_owned(),
                                    data_type: <crate::datatypes::Vec3D>::to_arrow_datatype(),
                                    is_nullable: true,
                                    metadata: [].into(),
                                },
                                Field {
                                    name: "rotation".to_owned(),
                                    data_type: <crate::datatypes::Rotation3D>::to_arrow_datatype(),
                                    is_nullable: true,
                                    metadata: [].into(),
                                },
                                Field {
                                    name: "scale".to_owned(),
                                    data_type: <crate::datatypes::Scale3D>::to_arrow_datatype(),
                                    is_nullable: true,
                                    metadata: [].into(),
                                },
                                Field {
                                    name: "from_parent".to_owned(),
                                    data_type: DataType::Boolean,
                                    is_nullable: false,
                                    metadata: [].into(),
                                },
                            ]),
                            "translation",
                        ))
                        .with_context("rerun.datatypes.TranslationRotationScale3D");
                    }
                    let data = &**arrays_by_name["translation"];
                    {
                        let data = data
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
                                    data.data_type().clone(),
                                )
                            })
                            .with_context(
                                "rerun.datatypes.TranslationRotationScale3D#translation",
                            )?;
                        if data.is_empty() {
                            Vec::new()
                        } else {
                            let offsets = (0..)
                                .step_by(3usize)
                                .zip((3usize..).step_by(3usize).take(data.len()));
                            let data_inner =
                                {
                                    let data_inner = &**data.values();
                                    data_inner
                                    .as_any()
                                    .downcast_ref::<Float32Array>()
                                    .ok_or_else(|| crate::DeserializationError::datatype_mismatch(
                                        DataType::Float32,
                                        data_inner.data_type().clone(),
                                    ))
                                    .with_context(
                                        "rerun.datatypes.TranslationRotationScale3D#translation",
                                    )?
                                    .into_iter()
                                    .map(|opt| opt.copied())
                                    .collect::<Vec<_>>()
                                };
                            arrow2::bitmap::utils::ZipValidity::new_with_validity(
                                offsets,
                                data.validity(),
                            )
                            .map(|elem| {
                                elem.map(|(start, end)| {
                                    debug_assert!(end - start == 3usize);
                                    if end as usize > data_inner.len() {
                                        return Err(crate::DeserializationError::offset_slice_oob(
                                            (start, end),
                                            data_inner.len(),
                                        ));
                                    }

                                    #[allow(unsafe_code, clippy::undocumented_unsafe_blocks)]
                                    let data = unsafe {
                                        data_inner.get_unchecked(start as usize..end as usize)
                                    };
                                    let data = data.iter().cloned().map(Option::unwrap_or_default);
                                    let arr = array_init::from_iter(data).unwrap();
                                    Ok(arr)
                                })
                                .transpose()
                            })
                            .map(|res_or_opt| {
                                res_or_opt.map(|res_or_opt| {
                                    res_or_opt.map(|v| crate::datatypes::Vec3D(v))
                                })
                            })
                            .collect::<crate::DeserializationResult<Vec<Option<_>>>>()?
                        }
                        .into_iter()
                    }
                };
                let rotation = {
                    if !arrays_by_name.contains_key("rotation") {
                        return Err(crate::DeserializationError::missing_struct_field(
                            DataType::Struct(vec![
                                Field {
                                    name: "translation".to_owned(),
                                    data_type: <crate::datatypes::Vec3D>::to_arrow_datatype(),
                                    is_nullable: true,
                                    metadata: [].into(),
                                },
                                Field {
                                    name: "rotation".to_owned(),
                                    data_type: <crate::datatypes::Rotation3D>::to_arrow_datatype(),
                                    is_nullable: true,
                                    metadata: [].into(),
                                },
                                Field {
                                    name: "scale".to_owned(),
                                    data_type: <crate::datatypes::Scale3D>::to_arrow_datatype(),
                                    is_nullable: true,
                                    metadata: [].into(),
                                },
                                Field {
                                    name: "from_parent".to_owned(),
                                    data_type: DataType::Boolean,
                                    is_nullable: false,
                                    metadata: [].into(),
                                },
                            ]),
                            "rotation",
                        ))
                        .with_context("rerun.datatypes.TranslationRotationScale3D");
                    }
                    let data = &**arrays_by_name["rotation"];
                    crate::datatypes::Rotation3D::try_from_arrow_opt(data)
                        .with_context("rerun.datatypes.TranslationRotationScale3D#rotation")?
                        .into_iter()
                };
                let scale = {
                    if !arrays_by_name.contains_key("scale") {
                        return Err(crate::DeserializationError::missing_struct_field(
                            DataType::Struct(vec![
                                Field {
                                    name: "translation".to_owned(),
                                    data_type: <crate::datatypes::Vec3D>::to_arrow_datatype(),
                                    is_nullable: true,
                                    metadata: [].into(),
                                },
                                Field {
                                    name: "rotation".to_owned(),
                                    data_type: <crate::datatypes::Rotation3D>::to_arrow_datatype(),
                                    is_nullable: true,
                                    metadata: [].into(),
                                },
                                Field {
                                    name: "scale".to_owned(),
                                    data_type: <crate::datatypes::Scale3D>::to_arrow_datatype(),
                                    is_nullable: true,
                                    metadata: [].into(),
                                },
                                Field {
                                    name: "from_parent".to_owned(),
                                    data_type: DataType::Boolean,
                                    is_nullable: false,
                                    metadata: [].into(),
                                },
                            ]),
                            "scale",
                        ))
                        .with_context("rerun.datatypes.TranslationRotationScale3D");
                    }
                    let data = &**arrays_by_name["scale"];
                    crate::datatypes::Scale3D::try_from_arrow_opt(data)
                        .with_context("rerun.datatypes.TranslationRotationScale3D#scale")?
                        .into_iter()
                };
                let from_parent = {
                    if !arrays_by_name.contains_key("from_parent") {
                        return Err(crate::DeserializationError::missing_struct_field(
                            DataType::Struct(vec![
                                Field {
                                    name: "translation".to_owned(),
                                    data_type: <crate::datatypes::Vec3D>::to_arrow_datatype(),
                                    is_nullable: true,
                                    metadata: [].into(),
                                },
                                Field {
                                    name: "rotation".to_owned(),
                                    data_type: <crate::datatypes::Rotation3D>::to_arrow_datatype(),
                                    is_nullable: true,
                                    metadata: [].into(),
                                },
                                Field {
                                    name: "scale".to_owned(),
                                    data_type: <crate::datatypes::Scale3D>::to_arrow_datatype(),
                                    is_nullable: true,
                                    metadata: [].into(),
                                },
                                Field {
                                    name: "from_parent".to_owned(),
                                    data_type: DataType::Boolean,
                                    is_nullable: false,
                                    metadata: [].into(),
                                },
                            ]),
                            "from_parent",
                        ))
                        .with_context("rerun.datatypes.TranslationRotationScale3D");
                    }
                    let data = &**arrays_by_name["from_parent"];
                    data.as_any()
                        .downcast_ref::<BooleanArray>()
                        .ok_or_else(|| {
                            crate::DeserializationError::datatype_mismatch(
                                DataType::Boolean,
                                data.data_type().clone(),
                            )
                        })
                        .with_context("rerun.datatypes.TranslationRotationScale3D#from_parent")?
                        .into_iter()
                };
                arrow2::bitmap::utils::ZipValidity::new_with_validity(
                    ::itertools::izip!(translation, rotation, scale, from_parent),
                    data.validity(),
                )
                .map(|opt| {
                    opt.map(|(translation, rotation, scale, from_parent)| {
                        Ok(Self {
                            translation,
                            rotation,
                            scale,
                            from_parent: from_parent
                                .ok_or_else(crate::DeserializationError::missing_data)
                                .with_context(
                                    "rerun.datatypes.TranslationRotationScale3D#from_parent",
                                )?,
                        })
                    })
                    .transpose()
                })
                .collect::<crate::DeserializationResult<Vec<_>>>()
                .with_context("rerun.datatypes.TranslationRotationScale3D")?
            }
        })
    }

    #[inline]
    fn try_iter_from_arrow(
        data: &dyn ::arrow2::array::Array,
    ) -> crate::DeserializationResult<Self::Iter<'_>>
    where
        Self: Sized,
    {
        Ok(Self::try_from_arrow_opt(data)?.into_iter())
    }

    #[inline]
    fn convert_item_to_opt_self(item: Self::Item<'_>) -> Option<Self> {
        item
    }
}

impl crate::Datatype for TranslationRotationScale3D {}
