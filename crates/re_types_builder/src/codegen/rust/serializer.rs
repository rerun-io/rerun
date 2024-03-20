use arrow2::datatypes::DataType;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use crate::{ArrowRegistry, Object, ObjectField, Objects};

use super::{
    arrow::{is_backed_by_arrow_buffer, quote_fqname_as_type_path},
    util::is_tuple_struct_from_obj,
};

// ---

pub fn quote_arrow_serializer(
    arrow_registry: &ArrowRegistry,
    objects: &Objects,
    obj: &Object,
    data_src: &proc_macro2::Ident,
) -> TokenStream {
    let datatype = &arrow_registry.get(&obj.fqname);

    let DataType::Extension(fqname, _, _) = datatype else {
        unreachable!()
    };
    let fqname_use = quote_fqname_as_type_path(fqname);
    let quoted_datatype = quote!(<#fqname_use>::arrow_datatype());

    let is_arrow_transparent = obj.datatype.is_none();
    let is_tuple_struct = is_tuple_struct_from_obj(obj);

    let quoted_flatten = |obj_field_is_nullable| {
        // NOTE: If the field itself is marked nullable, then we'll end up with two layers of
        // nullability in the output. Get rid of the superfluous one.
        if obj_field_is_nullable {
            quote!(.flatten())
        } else {
            quote!()
        }
    };

    let quoted_bitmap = |var| {
        quote! {
            let #var: Option<arrow2::bitmap::Bitmap> = {
                // NOTE: Don't compute a bitmap if there isn't at least one null element.
                let any_nones = somes.iter().any(|some| !*some);
                any_nones.then(|| somes.into())
            }
        }
    };

    if is_arrow_transparent {
        // NOTE: Arrow transparent objects must have a single field, no more no less.
        // The semantic pass would have failed already if this wasn't the case.
        let obj_field = &obj.fields[0];

        let quoted_data_src = data_src.clone();
        let quoted_data_dst = format_ident!(
            "{}",
            if is_tuple_struct {
                "data0"
            } else {
                obj_field.name.as_str()
            }
        );
        let bitmap_dst = format_ident!("{quoted_data_dst}_bitmap");

        let quoted_binding = if is_tuple_struct {
            quote!(Self(#quoted_data_dst))
        } else {
            quote!(Self { #quoted_data_dst })
        };

        let datatype = &arrow_registry.get(&obj_field.fqname);
        let quoted_datatype = quote! { Self::arrow_datatype() };

        let quoted_serializer = quote_arrow_field_serializer(
            objects,
            datatype,
            &quoted_datatype,
            obj_field.is_nullable,
            Some(obj_field),
            &bitmap_dst,
            &quoted_data_dst,
            InnerRepr::NativeIterable,
        );

        let quoted_bitmap = quoted_bitmap(bitmap_dst);

        let quoted_flatten = quoted_flatten(obj_field.is_nullable);

        quote! {{
            let (somes, #quoted_data_dst): (Vec<_>, Vec<_>) = #quoted_data_src
                .into_iter()
                .map(|datum| {
                    let datum: Option<::std::borrow::Cow<'a, Self>> = datum.map(Into::into);

                    let datum = datum
                        .map(|datum| {
                            let #quoted_binding = datum.into_owned();
                            #quoted_data_dst
                        })
                        #quoted_flatten;

                    (datum.is_some(), datum)
                })
                .unzip();


            #quoted_bitmap;

            #quoted_serializer
        }}
    } else {
        let data_src = data_src.clone();

        // NOTE: This can only be struct or union/enum at this point.
        match datatype.to_logical_type() {
            DataType::Struct(_) => {
                let quoted_field_serializers = obj.fields.iter().map(|obj_field| {
                    let data_dst = format_ident!("{}", obj_field.name);
                    let bitmap_dst = format_ident!("{data_dst}_bitmap");

                    let inner_datatype = &arrow_registry.get(&obj_field.fqname);
                    let quoted_inner_datatype =
                        super::arrow::ArrowDataTypeTokenizer(inner_datatype, false);

                    let quoted_serializer = quote_arrow_field_serializer(
                        objects,
                        inner_datatype,
                        &quoted_inner_datatype,
                        obj_field.is_nullable,
                        Some(obj_field),
                        &bitmap_dst,
                        &data_dst,
                        InnerRepr::NativeIterable,
                    );

                    let quoted_flatten = quoted_flatten(obj_field.is_nullable);

                    let quoted_bitmap = quoted_bitmap(bitmap_dst);

                    quote! {{
                        let (somes, #data_dst): (Vec<_>, Vec<_>) = #data_src
                            .iter()
                            .map(|datum| {
                                let datum = datum
                                    .as_ref()
                                    .map(|datum| {
                                        let Self { #data_dst, .. } = &**datum;
                                        #data_dst.clone()
                                    })
                                    #quoted_flatten;

                                (datum.is_some(), datum)
                            })
                            .unzip();


                        #quoted_bitmap;

                        #quoted_serializer
                    }}
                });

                let quoted_bitmap = quoted_bitmap(format_ident!("bitmap"));

                quote! {{
                    let (somes, #data_src): (Vec<_>, Vec<_>) = #data_src
                        .into_iter()
                        .map(|datum| {
                            let datum: Option<::std::borrow::Cow<'a, Self>> = datum.map(Into::into);
                            (datum.is_some(), datum)
                        })
                        .unzip();

                    #quoted_bitmap;

                    StructArray::new(
                        #quoted_datatype,
                        vec![#(#quoted_field_serializers,)*],
                        bitmap,
                    ).boxed()
                }}
            }

            DataType::Union(_, _, arrow2::datatypes::UnionMode::Sparse) => {
                // We use sparse unions for enums, which means only 8 bits is required for each field,
                // and nulls are encoded with a special 0-index `_null_markers` variant.

                let quoted_data_collect = quote! {
                    let #data_src: Vec<_> = #data_src
                        .into_iter()
                        .map(|datum| {
                            let datum: Option<::std::borrow::Cow<'a, Self>> = datum.map(Into::into);
                            datum
                        })
                        .collect();
                };

                let quoted_types = quote! {
                    #data_src
                        .iter()
                        .map(|a| match a.as_deref() {
                            None => 0,
                            Some(value) => *value as i8,
                        })
                        .collect()
                };

                let num_variants = obj.fields.len();

                quote! {{
                    #quoted_data_collect

                    let num_variants = #num_variants;

                    let types = #quoted_types;

                    let fields: Vec<_> = std::iter::repeat(
                            NullArray::new(
                                DataType::Null,
                                #data_src.len(),
                            ).boxed()
                        ).take(1 + num_variants) // +1 for the virtual `nulls` arm
                        .collect();

                    UnionArray::new(
                        #quoted_datatype,
                        types,
                        fields,
                        None,
                    ).boxed()
                }}
            }

            DataType::Union(_, _, arrow2::datatypes::UnionMode::Dense) => {
                let quoted_data_collect = quote! {
                    let #data_src: Vec<_> = #data_src
                        .into_iter()
                        .map(|datum| {
                            let datum: Option<::std::borrow::Cow<'a, Self>> = datum.map(Into::into);
                            datum
                        })
                        .collect();
                };

                let quoted_field_serializers = obj.fields.iter().map(|obj_field| {
                    let data_dst = format_ident!("{}", obj_field.snake_case_name());
                    let bitmap_dst = format_ident!("{data_dst}_bitmap");

                    let inner_datatype = &arrow_registry.get(&obj_field.fqname);
                    let quoted_inner_datatype = super::arrow::ArrowDataTypeTokenizer(inner_datatype, false);

                    let quoted_serializer = quote_arrow_field_serializer(
                        objects,
                        inner_datatype,
                        &quoted_inner_datatype,
                        obj_field.is_nullable,
                        Some(obj_field),
                        &bitmap_dst,
                        &data_dst,
                        InnerRepr::NativeIterable
                    );

                    let quoted_flatten = quoted_flatten(obj_field.is_nullable);
                    let quoted_bitmap = quoted_bitmap(bitmap_dst);

                    let quoted_obj_name = format_ident!("{}", obj.name);
                    let quoted_obj_field_name = format_ident!("{}", obj_field.pascal_case_name());

                    quote! {{
                        let (somes, #data_dst): (Vec<_>, Vec<_>) = #data_src
                            .iter()
                            .filter(|datum| matches!(datum.as_deref(), Some(#quoted_obj_name::#quoted_obj_field_name(_))))
                            .map(|datum| {
                                let datum = match datum.as_deref() {
                                    Some(#quoted_obj_name::#quoted_obj_field_name(v)) => Some(v.clone()),
                                    _ => None,
                                } #quoted_flatten ;

                                (datum.is_some(), datum)
                            })
                            .unzip();


                        #quoted_bitmap;

                        #quoted_serializer
                    }}
                });

                let quoted_fields = quote! {
                    vec![
                        NullArray::new(
                            DataType::Null,
                            #data_src.iter().filter(|v| v.is_none()).count(),
                        ).boxed(),
                        #(#quoted_field_serializers,)*
                    ]
                };

                let quoted_types = {
                    let quoted_obj_name = format_ident!("{}", obj.name);
                    let quoted_branches = obj.fields.iter().enumerate().map(|(i, obj_field)| {
                        let i = 1 + i as i8; // NOTE: +1 to account for `nulls` virtual arm
                        let quoted_obj_field_name =
                            format_ident!("{}", obj_field.pascal_case_name());

                        quote!(Some(#quoted_obj_name::#quoted_obj_field_name(_)) => #i)
                    });

                    quote! {
                        #data_src
                            .iter()
                            .map(|a| match a.as_deref() {
                                None => 0,
                                #(#quoted_branches,)*
                            })
                            .collect()
                    }
                };

                let quoted_offsets = {
                    let quoted_obj_name = format_ident!("{}", obj.name);

                    let quoted_counters = obj.fields.iter().map(|obj_field| {
                        let quoted_obj_field_name =
                            format_ident!("{}_offset", obj_field.snake_case_name());
                        quote!(let mut #quoted_obj_field_name = 0)
                    });

                    let quoted_branches = obj.fields.iter().map(|obj_field| {
                        let quoted_counter_name =
                            format_ident!("{}_offset", obj_field.snake_case_name());
                        let quoted_obj_field_name =
                            format_ident!("{}", obj_field.pascal_case_name());
                        quote! {
                            Some(#quoted_obj_name::#quoted_obj_field_name(_)) => {
                                let offset = #quoted_counter_name;
                                #quoted_counter_name += 1;
                                offset
                            }
                        }
                    });

                    quote! {{
                        #(#quoted_counters;)*
                        let mut nulls_offset = 0;

                        #data_src
                            .iter()
                            .map(|v| match v.as_deref() {
                                None => {
                                    let offset = nulls_offset;
                                    nulls_offset += 1;
                                    offset
                                }
                                #(#quoted_branches,)*
                            })
                            .collect()
                    }}
                };

                quote! {{
                    #quoted_data_collect

                    let types = #quoted_types;
                    let fields = #quoted_fields;
                    let offsets = Some(#quoted_offsets);

                    UnionArray::new(
                        #quoted_datatype,
                        types,
                        fields,
                        offsets,
                    ).boxed()
                }}
            }

            _ => unimplemented!("{datatype:#?}"),
        }
    }
}

#[derive(Copy, Clone)]
enum InnerRepr {
    /// The inner elements of the field will come from an `ArrowBuffer<T>`
    /// This is only applicable when T is an arrow primitive
    ArrowBuffer,

    /// The inner elements of the field will come from an iterable of T
    NativeIterable,
}

#[allow(clippy::too_many_arguments)]
fn quote_arrow_field_serializer(
    objects: &Objects,
    datatype: &DataType,
    quoted_datatype: &dyn quote::ToTokens,
    is_nullable: bool,
    obj_field: Option<&ObjectField>,
    bitmap_src: &proc_macro2::Ident,
    data_src: &proc_macro2::Ident,
    inner_repr: InnerRepr,
) -> TokenStream {
    let inner_obj = if let DataType::Extension(fqname, _, _) = datatype {
        Some(&objects[fqname])
    } else {
        None
    };
    let inner_is_arrow_transparent = inner_obj.map_or(false, |obj| obj.datatype.is_none());

    match datatype.to_logical_type() {
        DataType::Boolean
        | DataType::Int8
        | DataType::Int16
        | DataType::Int32
        | DataType::Int64
        | DataType::UInt8
        | DataType::UInt16
        | DataType::UInt32
        | DataType::UInt64
        | DataType::Float16
        | DataType::Float32
        | DataType::Float64 => {
            // NOTE: We need values for all slots, regardless of what the bitmap says,
            // hence `unwrap_or_default`.
            let quoted_transparent_mapping = if inner_is_arrow_transparent {
                let inner_obj = inner_obj.as_ref().unwrap();
                let quoted_inner_obj_type = quote_fqname_as_type_path(&inner_obj.fqname);
                let is_tuple_struct = is_tuple_struct_from_obj(inner_obj);
                let quoted_data_dst = format_ident!(
                    "{}",
                    if is_tuple_struct {
                        "data0"
                    } else {
                        inner_obj.fields[0].name.as_str()
                    }
                );
                let quoted_binding = if is_tuple_struct {
                    quote!(#quoted_inner_obj_type(#quoted_data_dst))
                } else {
                    quote!(#quoted_inner_obj_type { #quoted_data_dst })
                };

                quote! {
                    .map(|datum| {
                        datum
                            .map(|datum| {
                                let #quoted_binding = datum;
                                #quoted_data_dst
                            })
                            .unwrap_or_default()
                    })
                }
            } else {
                quote! {
                    .map(|v| v.unwrap_or_default())
                }
            };

            if datatype.to_logical_type() == &DataType::Boolean {
                quote! {
                    BooleanArray::new(
                        #quoted_datatype,
                        // NOTE: We need values for all slots, regardless of what the bitmap says,
                        // hence `unwrap_or_default`.
                        #data_src.into_iter() #quoted_transparent_mapping .collect(),
                        #bitmap_src,
                    ).boxed()
                }
            } else {
                match inner_repr {
                    // A primitive that's an inner element of a list will already have been mapped
                    // to a buffer type.
                    InnerRepr::ArrowBuffer => quote! {
                        PrimitiveArray::new(
                            #quoted_datatype,
                            #data_src,
                            #bitmap_src,
                        ).boxed()
                    },
                    InnerRepr::NativeIterable => quote! {
                        PrimitiveArray::new(
                            #quoted_datatype,
                            #data_src.into_iter() #quoted_transparent_mapping .collect(),
                            #bitmap_src,
                        ).boxed()
                    },
                }
            }
        }

        DataType::Utf8 => {
            // NOTE: We need values for all slots, regardless of what the bitmap says,
            // hence `unwrap_or_default`.
            let (quoted_transparent_mapping, quoted_transparent_length) =
                if inner_is_arrow_transparent {
                    let inner_obj = inner_obj.as_ref().unwrap();
                    let quoted_inner_obj_type = quote_fqname_as_type_path(&inner_obj.fqname);
                    let is_tuple_struct = is_tuple_struct_from_obj(inner_obj);
                    let quoted_data_dst = format_ident!(
                        "{}",
                        if is_tuple_struct {
                            "data0"
                        } else {
                            inner_obj.fields[0].name.as_str()
                        }
                    );
                    let quoted_binding = if is_tuple_struct {
                        quote!(#quoted_inner_obj_type(#quoted_data_dst))
                    } else {
                        quote!(#quoted_inner_obj_type { #quoted_data_dst })
                    };

                    (
                        quote! {
                            .flat_map(|datum| {
                                let #quoted_binding = datum;
                                // NOTE: `Buffer::clone`, which is just a ref-count bump
                                #quoted_data_dst .0.clone()
                            })
                        },
                        quote! {
                            .map(|datum| {
                                let #quoted_binding = datum;
                                #quoted_data_dst.0.len()
                            }).unwrap_or_default()
                        },
                    )
                } else {
                    (
                        quote! {
                            // NOTE: `Buffer::clone`, which is just a ref-count bump
                            .flat_map(|s| s.0.clone())
                        },
                        quote! {
                            .map(|datum| datum.0.len()).unwrap_or_default()
                        },
                    )
                };

            quote! {{
                // NOTE: Flattening to remove the guaranteed layer of nullability: we don't care
                // about it while building the backing buffer since it's all offsets driven.
                let inner_data: arrow2::buffer::Buffer<u8> =
                    #data_src.iter().flatten() #quoted_transparent_mapping.collect();

                let offsets = arrow2::offset::Offsets::<i32>::try_from_lengths(
                    #data_src.iter().map(|opt| opt.as_ref() #quoted_transparent_length )
                ).unwrap().into();

                // Safety: we're building this from actual native strings, so no need to do the
                // whole utf8 validation _again_.
                #[allow(unsafe_code, clippy::undocumented_unsafe_blocks)]
                unsafe { Utf8Array::<i32>::new_unchecked(#quoted_datatype, offsets, inner_data, #bitmap_src) }.boxed()
            }}
        }

        DataType::List(inner) | DataType::FixedSizeList(inner, _) => {
            let inner_datatype = inner.data_type();
            let quoted_inner_datatype = super::arrow::ArrowDataTypeTokenizer(inner_datatype, false);

            // Note: We only use the ArrowBuffer optimization for `Lists` but not `FixedSizeList`.
            // This is because the `ArrowBuffer` has a dynamic length, which would add more overhead
            // to simple fixed-sized types like `Position2D`.
            //
            // TODO(jleibs): If we need to support large FixedSizeList types where the `ArrowBuffer`
            // optimization would be significant, we can introduce a new attribute to force this.
            let inner_repr = if is_backed_by_arrow_buffer(inner.data_type())
                && matches!(datatype, DataType::List(_))
            {
                InnerRepr::ArrowBuffer
            } else {
                InnerRepr::NativeIterable
            };

            let quoted_inner_data = format_ident!("{data_src}_inner_data");
            let quoted_inner_bitmap = format_ident!("{data_src}_inner_bitmap");

            let quoted_inner = quote_arrow_field_serializer(
                objects,
                inner_datatype,
                &quoted_inner_datatype,
                inner.is_nullable,
                None,
                &quoted_inner_bitmap,
                &quoted_inner_data,
                inner_repr,
            );

            let serde_type = obj_field.and_then(|obj_field| {
                obj_field.try_get_attr::<String>(crate::ATTR_RUST_SERDE_TYPE)
            });

            let quoted_transparent_mapping = if inner_is_arrow_transparent {
                let inner_obj = inner_obj.as_ref().unwrap();
                let quoted_inner_obj_type = quote_fqname_as_type_path(&inner_obj.fqname);
                let is_tuple_struct = is_tuple_struct_from_obj(inner_obj);
                let quoted_data_dst = format_ident!(
                    "{}",
                    if is_tuple_struct {
                        "data0"
                    } else {
                        inner_obj.fields[0].name.as_str()
                    }
                );
                let quoted_binding = if is_tuple_struct {
                    quote!(#quoted_inner_obj_type(#quoted_data_dst))
                } else {
                    quote!(#quoted_inner_obj_type { #quoted_data_dst })
                };

                quote! {
                    .map(|datum| {
                        datum
                            .map(|datum| {
                                let #quoted_binding = datum;
                                #quoted_data_dst
                            })
                            .unwrap_or_default()
                    })
                    // NOTE: Flattening yet again since we have to deconstruct the inner list.
                    .flatten()
                }
            } else {
                match inner_repr {
                    InnerRepr::ArrowBuffer => {
                        if serde_type.is_some() {
                            quote! {
                                .map(|opt| {
                                    use ::re_types_core::SerializationError; // otherwise rustfmt breaks
                                    opt.as_ref().map(|b| {
                                        let mut buf = Vec::new();
                                        rmp_serde::encode::write_named(&mut buf, b)
                                            .map_err(|err| SerializationError::serde_failure(err.to_string()))?;
                                        Ok(buf)
                                    })
                                    .transpose()
                                })
                                .collect::<SerializationResult<Vec<_>>>()?
                            }
                        } else {
                            quote! {
                                .flatten()
                                .map(|b| b.as_slice())
                                .collect::<Vec<_>>()
                                .concat()
                                .into();
                            }
                        }
                    }
                    InnerRepr::NativeIterable => {
                        if let DataType::FixedSizeList(_, count) = datatype.to_logical_type() {
                            quote! {
                                .flat_map(|v| match v {
                                    Some(v) => itertools::Either::Left(v.iter().cloned()),
                                    None => itertools::Either::Right(
                                        std::iter::repeat(Default::default()).take(#count),
                                    ),
                                })
                            }
                        } else {
                            quote! {
                                .flatten()
                                // NOTE: Flattening yet again since we have to deconstruct the inner list.
                                .flatten()
                                .cloned()
                            }
                        }
                    }
                }
            };

            let quoted_num_instances = match inner_repr {
                InnerRepr::ArrowBuffer => quote!(num_instances()),
                InnerRepr::NativeIterable => quote!(len()),
            };

            let quoted_create = if let DataType::List(_) = datatype {
                if serde_type.is_some() {
                    quote! {}
                } else {
                    quote! {
                        let offsets = arrow2::offset::Offsets::<i32>::try_from_lengths(
                            #data_src.iter().map(|opt| opt.as_ref().map(|datum| datum. #quoted_num_instances).unwrap_or_default())
                        ).unwrap().into();

                        ListArray::new(
                            #quoted_datatype,
                            offsets,
                            #quoted_inner,
                            #bitmap_src,
                        ).boxed()
                    }
                }
            } else {
                quote! {
                    FixedSizeListArray::new(
                        #quoted_datatype,
                        #quoted_inner,
                        #bitmap_src,
                    ).boxed()
                }
            };

            // TODO(#2993): The inner
            // types of lists shouldn't be nullable, but both the python and C++
            // code-gen end up setting these to null when an outer fixed-sized
            // field does happen to be null. In order to keep everything aligned
            // at a validation level we match this behavior and create a
            // validity-mask for the corresponding inner type. We can undo this
            // if we make the C++ and Python codegen match the rust behavior or
            // make our comparison tests more lenient.
            let quoted_inner_bitmap =
                if let DataType::FixedSizeList(_, count) = datatype.to_logical_type() {
                    quote! {
                        let #quoted_inner_bitmap: Option<arrow2::bitmap::Bitmap> =
                            #bitmap_src.as_ref().map(|bitmap| {
                                bitmap
                                    .iter()
                                    .map(|i| std::iter::repeat(i).take(#count))
                                    .flatten()
                                    .collect::<Vec<_>>()
                                    .into()
                            });
                    }
                } else {
                    // TODO(cmc): We don't support intra-list nullability in our IDL at the moment.
                    quote! {
                        let #quoted_inner_bitmap: Option<arrow2::bitmap::Bitmap> = None;
                    }
                };

            // TODO(cmc): We should be checking this, but right now we don't because we don't
            // support intra-list nullability.
            _ = is_nullable;
            match inner_repr {
                InnerRepr::ArrowBuffer => {
                    if serde_type.is_some() {
                        quote! {{
                            use arrow2::{buffer::Buffer, offset::OffsetsBuffer};

                            let buffers: Vec<Option<Vec<u8>>> = #data_src
                                .iter()
                                #quoted_transparent_mapping;

                            let offsets = arrow2::offset::Offsets::<i32>::try_from_lengths(
                                buffers.iter().map(|opt| opt.as_ref().map(|buf| buf.len()).unwrap_or_default())
                            ).unwrap().into();

                            #quoted_inner_bitmap

                            let #quoted_inner_data: Buffer<u8> = buffers.into_iter().flatten().collect::<Vec<_>>().concat().into();

                            ListArray::new(
                                #quoted_datatype,
                                offsets,
                                #quoted_inner,
                                #bitmap_src,
                            ).boxed()
                        }}
                    } else {
                        quote! {{
                            use arrow2::{buffer::Buffer, offset::OffsetsBuffer};

                            let #quoted_inner_data: Buffer<_> = #data_src
                                .iter()
                                #quoted_transparent_mapping

                            #quoted_inner_bitmap

                            #quoted_create
                        }}
                    }
                }
                InnerRepr::NativeIterable => quote! {{
                    use arrow2::{buffer::Buffer, offset::OffsetsBuffer};

                    let #quoted_inner_data: Vec<_> = #data_src
                        .iter()
                        #quoted_transparent_mapping
                        // NOTE: Wrapping back into an option as the recursive call will expect the
                        // guaranteed nullability layer to be present!
                        .map(Some)
                        .collect();

                    #quoted_inner_bitmap

                    #quoted_create
                }},
            }
        }

        DataType::Struct(_) | DataType::Union(_, _, _) => {
            // NOTE: We always wrap objects with full extension metadata.
            let DataType::Extension(fqname, _, _) = datatype else {
                unreachable!()
            };
            let fqname_use = quote_fqname_as_type_path(fqname);
            quote! {{
                _ = #bitmap_src;
                #fqname_use::to_arrow_opt(#data_src)?
            }}
        }

        _ => unimplemented!("{datatype:#?}"),
    }
}
