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

#[doc = "An affine transform between two 3D spaces, represented in a given direction."]
#[derive(Clone, Debug)]
pub struct Transform3D {
    #[doc = "Representation of the transform."]
    pub repr: crate::datatypes::Transform3D,
}

impl<'a> From<Transform3D> for ::std::borrow::Cow<'a, Transform3D> {
    #[inline]
    fn from(value: Transform3D) -> Self {
        std::borrow::Cow::Owned(value)
    }
}

impl<'a> From<&'a Transform3D> for ::std::borrow::Cow<'a, Transform3D> {
    #[inline]
    fn from(value: &'a Transform3D) -> Self {
        std::borrow::Cow::Borrowed(value)
    }
}

impl crate::Component for Transform3D {
    #[inline]
    fn name() -> crate::ComponentName {
        crate::ComponentName::Borrowed("rerun.components.Transform3D")
    }

    #[allow(unused_imports, clippy::wildcard_imports)]
    #[inline]
    fn to_arrow_datatype() -> arrow2::datatypes::DataType {
        use ::arrow2::datatypes::*;
        DataType::Union(
            vec![
                Field {
                    name: "TranslationAndMat3x3".to_owned(),
                    data_type: DataType::Struct(vec![
                        Field {
                            name: "translation".to_owned(),
                            data_type: DataType::FixedSizeList(
                                Box::new(Field {
                                    name: "item".to_owned(),
                                    data_type: DataType::Float32,
                                    is_nullable: false,
                                    metadata: [].into(),
                                }),
                                3usize,
                            ),
                            is_nullable: true,
                            metadata: [].into(),
                        },
                        Field {
                            name: "matrix".to_owned(),
                            data_type: DataType::FixedSizeList(
                                Box::new(Field {
                                    name: "item".to_owned(),
                                    data_type: DataType::Float32,
                                    is_nullable: false,
                                    metadata: [].into(),
                                }),
                                9usize,
                            ),
                            is_nullable: true,
                            metadata: [].into(),
                        },
                        Field {
                            name: "from_parent".to_owned(),
                            data_type: DataType::Boolean,
                            is_nullable: true,
                            metadata: [].into(),
                        },
                    ]),
                    is_nullable: false,
                    metadata: [].into(),
                },
                Field {
                    name: "TranslationRotationScale".to_owned(),
                    data_type: DataType::Struct(vec![
                        Field {
                            name: "translation".to_owned(),
                            data_type: DataType::FixedSizeList(
                                Box::new(Field {
                                    name: "item".to_owned(),
                                    data_type: DataType::Float32,
                                    is_nullable: false,
                                    metadata: [].into(),
                                }),
                                3usize,
                            ),
                            is_nullable: true,
                            metadata: [].into(),
                        },
                        Field {
                            name: "rotation".to_owned(),
                            data_type: DataType::Union(
                                vec![
                                    Field {
                                        name: "Quaternion".to_owned(),
                                        data_type: DataType::FixedSizeList(
                                            Box::new(Field {
                                                name: "item".to_owned(),
                                                data_type: DataType::Float32,
                                                is_nullable: false,
                                                metadata: [].into(),
                                            }),
                                            4usize,
                                        ),
                                        is_nullable: false,
                                        metadata: [].into(),
                                    },
                                    Field {
                                        name: "AxisAngle".to_owned(),
                                        data_type: DataType::Struct(vec![
                                            Field {
                                                name: "axis".to_owned(),
                                                data_type: DataType::FixedSizeList(
                                                    Box::new(Field {
                                                        name: "item".to_owned(),
                                                        data_type: DataType::Float32,
                                                        is_nullable: false,
                                                        metadata: [].into(),
                                                    }),
                                                    3usize,
                                                ),
                                                is_nullable: false,
                                                metadata: [].into(),
                                            },
                                            Field {
                                                name: "angle".to_owned(),
                                                data_type: DataType::Union(
                                                    vec![
                                                        Field {
                                                            name: "Radians".to_owned(),
                                                            data_type: DataType::Float32,
                                                            is_nullable: false,
                                                            metadata: [].into(),
                                                        },
                                                        Field {
                                                            name: "Degrees".to_owned(),
                                                            data_type: DataType::Float32,
                                                            is_nullable: false,
                                                            metadata: [].into(),
                                                        },
                                                    ],
                                                    None,
                                                    UnionMode::Dense,
                                                ),
                                                is_nullable: false,
                                                metadata: [].into(),
                                            },
                                        ]),
                                        is_nullable: false,
                                        metadata: [].into(),
                                    },
                                ],
                                None,
                                UnionMode::Dense,
                            ),
                            is_nullable: true,
                            metadata: [].into(),
                        },
                        Field {
                            name: "scale".to_owned(),
                            data_type: DataType::Union(
                                vec![
                                    Field {
                                        name: "ThreeD".to_owned(),
                                        data_type: DataType::FixedSizeList(
                                            Box::new(Field {
                                                name: "item".to_owned(),
                                                data_type: DataType::Float32,
                                                is_nullable: false,
                                                metadata: [].into(),
                                            }),
                                            3usize,
                                        ),
                                        is_nullable: false,
                                        metadata: [].into(),
                                    },
                                    Field {
                                        name: "Uniform".to_owned(),
                                        data_type: DataType::Float32,
                                        is_nullable: false,
                                        metadata: [].into(),
                                    },
                                ],
                                None,
                                UnionMode::Dense,
                            ),
                            is_nullable: true,
                            metadata: [].into(),
                        },
                        Field {
                            name: "from_parent".to_owned(),
                            data_type: DataType::Boolean,
                            is_nullable: true,
                            metadata: [].into(),
                        },
                    ]),
                    is_nullable: false,
                    metadata: [].into(),
                },
            ],
            None,
            UnionMode::Dense,
        )
    }

    #[allow(unused_imports, clippy::wildcard_imports)]
    fn try_to_arrow_opt<'a>(
        data: impl IntoIterator<Item = Option<impl Into<::std::borrow::Cow<'a, Self>>>>,
        extension_wrapper: Option<&str>,
    ) -> crate::SerializationResult<Box<dyn ::arrow2::array::Array>>
    where
        Self: Clone + 'a,
    {
        use crate::{Component as _, Datatype as _};
        use ::arrow2::{array::*, datatypes::*};
        Ok({
            let (somes, repr): (Vec<_>, Vec<_>) = data
                .into_iter()
                .map(|datum| {
                    let datum: Option<::std::borrow::Cow<'a, Self>> = datum.map(Into::into);
                    let datum = datum.map(|datum| {
                        let Self { repr } = datum.into_owned();
                        repr
                    });
                    (datum.is_some(), datum)
                })
                .unzip();
            let repr_bitmap: Option<::arrow2::bitmap::Bitmap> = {
                let any_nones = somes.iter().any(|some| !*some);
                any_nones.then(|| somes.into())
            };
            {
                _ = repr_bitmap;
                _ = extension_wrapper;
                crate::datatypes::Transform3D::try_to_arrow_opt(
                    repr,
                    Some("rerun.components.Transform3D"),
                )?
            }
        })
    }

    #[allow(unused_imports, clippy::wildcard_imports)]
    fn try_from_arrow_opt(
        data: &dyn ::arrow2::array::Array,
    ) -> crate::DeserializationResult<Vec<Option<Self>>>
    where
        Self: Sized,
    {
        use crate::{Component as _, Datatype as _};
        use ::arrow2::{array::*, datatypes::*};
        Ok(crate::datatypes::Transform3D::try_from_arrow_opt(data)?
            .into_iter()
            .map(|v| {
                v.ok_or_else(|| crate::DeserializationError::MissingData {
                    datatype: data.data_type().clone(),
                })
            })
            .map(|res| res.map(|repr| Some(Self { repr })))
            .collect::<crate::DeserializationResult<Vec<Option<_>>>>()?)
    }
}
