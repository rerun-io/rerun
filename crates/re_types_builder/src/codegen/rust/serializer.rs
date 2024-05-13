use arrow2::datatypes::DataType;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use crate::{ArrowRegistry, Object, ObjectField, Objects};

use super::{
    arrow::{is_backed_by_arrow_buffer, quote_fqname_as_type_path},
    util::{is_tuple_struct_from_obj, quote_comment},
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

        let quoted_member_accessor = if is_tuple_struct {
            quote!(0)
        } else {
            quote!(#quoted_data_dst)
        };

        let datatype = &arrow_registry.get(&obj_field.fqname);
        let quoted_datatype = quote! { Self::arrow_datatype() };
        let elements_are_nullable = true;

        let quoted_serializer = quote_arrow_field_serializer(
            objects,
            datatype,
            &quoted_datatype,
            Some(obj_field),
            &bitmap_dst,
            elements_are_nullable,
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
                            datum.into_owned().#quoted_member_accessor
                        })
                        #quoted_flatten;

                    (datum.is_some(), datum)
                })
                .unzip();


            #quoted_bitmap;

            #quoted_serializer
        }}
    } else {
        // NOTE: This can only be struct or union/enum at this point.
        match datatype.to_logical_type() {
            DataType::Struct(_) => {
                let quoted_field_serializers = obj.fields.iter().map(|obj_field| {
                    let data_dst = format_ident!("{}", obj_field.name);
                    let bitmap_dst = format_ident!("{data_dst}_bitmap");

                    let inner_datatype = &arrow_registry.get(&obj_field.fqname);
                    let quoted_inner_datatype =
                        super::arrow::ArrowDataTypeTokenizer(inner_datatype, false);
                    let elements_are_nullable = true;

                    let quoted_serializer = quote_arrow_field_serializer(
                        objects,
                        inner_datatype,
                        &quoted_inner_datatype,
                        Some(obj_field),
                        &bitmap_dst,
                        elements_are_nullable,
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
                                        datum.#data_dst.clone()
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

                let comment = quote_comment("Sparse Arrow union");

                quote! {{
                    #comment

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

                let quoted_obj_name = format_ident!("{}", obj.name);

                let quoted_field_serializers = obj.fields.iter().map(|obj_field| {
                    let quoted_obj_field_name = format_ident!("{}", obj_field.pascal_case_name());

                    // Short circuit for empty variants since they're trivial to solve at this level:
                    if obj_field.typ == crate::Type::Unit {
                        return quote! {
                            NullArray::new(
                                DataType::Null,
                                #data_src
                                    .iter()
                                    .filter(|datum| matches!(datum.as_deref(), Some(#quoted_obj_name::#quoted_obj_field_name)))
                                    .count(),
                            ).boxed()
                        };
                    }

                    let data_dst = format_ident!("{}", obj_field.snake_case_name());

                    // We handle nullability with a special null variant that is always present.
                    let elements_are_nullable = false;
                    let bitmap_dst = format_ident!("{}_bitmap", data_dst);

                    let inner_datatype = &arrow_registry.get(&obj_field.fqname);
                    let quoted_inner_datatype =
                        super::arrow::ArrowDataTypeTokenizer(inner_datatype, false);

                    let quoted_serializer = quote_arrow_field_serializer(
                        objects,
                        inner_datatype,
                        &quoted_inner_datatype,
                        Some(obj_field),
                        &bitmap_dst,
                        elements_are_nullable,
                        &data_dst,
                        InnerRepr::NativeIterable,
                    );

                    quote! {{
                        let #data_dst: Vec<_> = data
                            .iter()
                            .filter_map(|datum| match datum.as_deref() {
                                Some(#quoted_obj_name::#quoted_obj_field_name(v)) => Some(v.clone()),
                                _ => None,
                            })
                            .collect();

                        let #bitmap_dst: Option<arrow2::bitmap::Bitmap> = None;
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

                let get_match_case_for_field = |typ, quoted_obj_field_name| {
                    if typ == &crate::Type::Unit {
                        quote!(Some(#quoted_obj_name::#quoted_obj_field_name))
                    } else {
                        quote!(Some(#quoted_obj_name::#quoted_obj_field_name(_)))
                    }
                };

                let quoted_types = {
                    let quoted_branches = obj.fields.iter().enumerate().map(|(i, obj_field)| {
                        let i = 1 + i as i8; // NOTE: +1 to account for `nulls` virtual arm
                        let quoted_obj_field_name =
                            format_ident!("{}", obj_field.pascal_case_name());
                        let match_case =
                            get_match_case_for_field(&obj_field.typ, quoted_obj_field_name);
                        quote!(#match_case => #i)
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

                        let match_case =
                            get_match_case_for_field(&obj_field.typ, quoted_obj_field_name);
                        quote! {
                            #match_case => {
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

                let comment = quote_comment("Dense Arrow union");

                quote! {{
                    #comment

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

/// Writes out code to serialize a single field.
///
/// If `elements_are_nullable` is `false`, then we ignore null elements in the input data.
/// This is useful for:
/// * unions: nullability is encoded as a separate variant
/// * lists inside of fields that are lists: we don't support intra-list nullability
/// TODO(#2993): However, we still emit a validity bitmap for lists inside lists
/// to `validity_bitmap_ident`since Python and Rust do so.
#[allow(clippy::too_many_arguments)]
fn quote_arrow_field_serializer(
    objects: &Objects,
    datatype: &DataType,
    quoted_datatype: &dyn quote::ToTokens,
    obj_field: Option<&ObjectField>,
    bitmap_src: &proc_macro2::Ident,
    elements_are_nullable: bool,
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
            // hence `unwrap_or_default` (unless elements_are_nullable is false)
            let quoted_transparent_mapping = if inner_is_arrow_transparent {
                let inner_obj = inner_obj.as_ref().unwrap();
                let quoted_inner_obj_type = quote_fqname_as_type_path(&inner_obj.fqname);
                let is_tuple_struct = is_tuple_struct_from_obj(inner_obj);
                let quoted_member_accessor = if is_tuple_struct {
                    quote!(0)
                } else {
                    quote!(#quoted_inner_obj_type)
                };

                if elements_are_nullable {
                    quote! {
                        .map(|datum| {
                            datum
                                .map(|datum| {
                                    datum.#quoted_member_accessor
                                })
                                .unwrap_or_default()
                        })
                    }
                } else {
                    quote! {
                        .map(|datum| {
                            datum.#quoted_member_accessor
                        })
                    }
                }
            } else if elements_are_nullable {
                quote! {
                    .map(|v| v.unwrap_or_default())
                }
            } else {
                quote! {}
            };

            if datatype.to_logical_type() == &DataType::Boolean {
                quote! {
                    BooleanArray::new(
                        #quoted_datatype,
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

        DataType::Null => {
            panic!("Null fields should only occur within enums and unions where they are handled separately.");
        }

        DataType::Utf8 => {
            // NOTE: We need values for all slots, regardless of what the bitmap says,
            // hence `unwrap_or_default`.
            let (quoted_transparent_mapping, quoted_transparent_length) =
                if inner_is_arrow_transparent {
                    let inner_obj = inner_obj.as_ref().unwrap();
                    let is_tuple_struct = is_tuple_struct_from_obj(inner_obj);
                    let quoted_data_dst = format_ident!(
                        "{}",
                        if is_tuple_struct {
                            "data0"
                        } else {
                            inner_obj.fields[0].name.as_str()
                        }
                    );
                    let quoted_member_accessor = if is_tuple_struct {
                        quote!(0)
                    } else {
                        quote!(#quoted_data_dst)
                    };

                    (
                        quote! {
                            .flat_map(|datum| {
                                datum.#quoted_member_accessor.0
                            })
                        },
                        quote! {
                            .map(|datum| {
                                datum.#quoted_member_accessor.len()
                            })
                        },
                    )
                } else {
                    (
                        quote! {
                            .flat_map(|s| s.0)
                        },
                        quote! {
                            .map(|datum| datum.len())
                        },
                    )
                };

            let inner_data_and_offsets = if elements_are_nullable {
                quote! {
                    let offsets = arrow2::offset::Offsets::<i32>::try_from_lengths(
                        #data_src.iter().map(|opt| opt.as_ref() #quoted_transparent_length .unwrap_or_default())
                    )
                    .map_err(|err| std::sync::Arc::new(err))?
                    .into();

                    // NOTE: Flattening to remove the guaranteed layer of nullability: we don't care
                    // about it while building the backing buffer since it's all offsets driven.
                    let inner_data: arrow2::buffer::Buffer<u8> =
                        #data_src.into_iter().flatten() #quoted_transparent_mapping.collect();
                }
            } else {
                quote! {
                    let offsets = arrow2::offset::Offsets::<i32>::try_from_lengths(
                        #data_src.iter() #quoted_transparent_length
                    )
                    .map_err(|err| std::sync::Arc::new(err))?
                    .into();

                    let inner_data: arrow2::buffer::Buffer<u8> =
                        #data_src.into_iter() #quoted_transparent_mapping.collect();
                }
            };

            quote! {{
                #inner_data_and_offsets

                // Safety: we're building this from actual native strings, so no need to do the
                // whole utf8 validation _again_.
                // It would be nice to use quote_comment here and put this safety notice in the generated code,
                // but that seems to push us over some complexity limit causing rustfmt to fail.
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
            let inner_bitmap_ident = format_ident!("{data_src}_inner_bitmap");
            let inner_elements_are_nullable = false; // We don't support intra-list nullability.

            let quoted_inner = quote_arrow_field_serializer(
                objects,
                inner_datatype,
                &quoted_inner_datatype,
                None,
                &inner_bitmap_ident,
                inner_elements_are_nullable,
                &quoted_inner_data,
                inner_repr,
            );

            let serde_type = obj_field.and_then(|obj_field| {
                obj_field.try_get_attr::<String>(crate::ATTR_RUST_SERDE_TYPE)
            });

            let quoted_transparent_mapping = if inner_is_arrow_transparent {
                let inner_obj = inner_obj.as_ref().unwrap();
                let is_tuple_struct = is_tuple_struct_from_obj(inner_obj);
                let quoted_data_dst = format_ident!(
                    "{}",
                    if is_tuple_struct {
                        "data0"
                    } else {
                        inner_obj.fields[0].name.as_str()
                    }
                );
                let quoted_member_accessor = if is_tuple_struct {
                    quote!(0)
                } else {
                    quote!(#quoted_data_dst)
                };

                if elements_are_nullable {
                    quote! {
                        #data_src
                        .into_iter()
                        .map(|datum| {
                            datum
                                .map(|datum| {
                                    datum.#quoted_member_accessor
                                })
                                .unwrap_or_default()
                        })
                        // NOTE: Flattening yet again since we have to deconstruct the inner list.
                        .flatten()
                    }
                } else {
                    quote! {
                        #data_src
                        .into_iter()
                        .map(|datum| {
                            datum.#quoted_member_accessor
                        })
                        // NOTE: Flattening yet since we have to deconstruct the inner list.
                        .flatten()
                    }
                }
            } else {
                let flatten_if_needed = if elements_are_nullable {
                    quote!( .flatten() )
                } else {
                    quote!()
                };

                match inner_repr {
                    InnerRepr::ArrowBuffer => {
                        if serde_type.is_some() {
                            quote! {
                                #data_src
                                .into_iter()
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
                            // TODO(emilk): this can probably be optimized
                            quote! {
                                #data_src
                                .iter()
                                #flatten_if_needed
                                .map(|b| b.as_slice())
                                .collect::<Vec<_>>()
                                .concat()
                                .into()
                            }
                        }
                    }
                    InnerRepr::NativeIterable => {
                        if let DataType::FixedSizeList(_, count) = datatype.to_logical_type() {
                            if elements_are_nullable {
                                quote! {
                                    #data_src
                                    .into_iter()
                                    .flat_map(|v| match v {
                                        Some(v) => itertools::Either::Left(v.into_iter()),
                                        None => itertools::Either::Right(
                                            std::iter::repeat(Default::default()).take(#count),
                                        ),
                                    })
                                }
                            } else {
                                quote! {
                                    #data_src
                                    .into_iter()
                                    .flatten()
                                }
                            }
                        } else {
                            quote! {
                                #data_src
                                .into_iter()
                                #flatten_if_needed
                                // NOTE: Flattening yet again since we have to deconstruct the inner list.
                                .flatten()
                            }
                        }
                    }
                }
            };

            let quoted_num_instances = match inner_repr {
                InnerRepr::ArrowBuffer => quote!(num_instances()),
                InnerRepr::NativeIterable => quote!(len()),
            };

            let quoted_declare_offsets = if let DataType::List(_) = datatype {
                if serde_type.is_some() {
                    quote! {}
                } else {
                    let map_to_length = if elements_are_nullable {
                        quote! { map(|opt| opt.as_ref().map_or(0, |datum| datum. #quoted_num_instances)) }
                    } else {
                        quote! { map(|datum| datum. #quoted_num_instances) }
                    };

                    quote! {
                        let offsets = arrow2::offset::Offsets::<i32>::try_from_lengths(
                            #data_src.iter(). #map_to_length
                        ).unwrap().into();
                    }
                }
            } else {
                quote! {}
            };

            let quoted_create = if let DataType::List(_) = datatype {
                if serde_type.is_some() {
                    quote! {}
                } else {
                    quote! {
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
            //
            // This workaround does not apply if we don't have any validity bitmap on the outer type.
            // (as it is always the case with unions where the nullability is encoded as a separate variant)
            let quoted_inner_bitmap = if let (true, DataType::FixedSizeList(_, count)) =
                (elements_are_nullable, datatype.to_logical_type())
            {
                quote! {
                    let #inner_bitmap_ident: Option<arrow2::bitmap::Bitmap> =
                        #bitmap_src.as_ref().map(|bitmap| {
                            bitmap
                                .iter()
                                .map(|b| std::iter::repeat(b).take(#count))
                                .flatten()
                                .collect::<Vec<_>>()
                                .into()
                        });
                }
            } else {
                // TODO(cmc): We don't support intra-list nullability in our IDL at the moment.
                quote! {
                    let #inner_bitmap_ident: Option<arrow2::bitmap::Bitmap> = None;
                }
            };

            match inner_repr {
                InnerRepr::ArrowBuffer => {
                    if serde_type.is_some() {
                        quote! {{
                            use arrow2::{buffer::Buffer, offset::OffsetsBuffer};

                            let buffers: Vec<Option<Vec<u8>>> = #quoted_transparent_mapping;

                            let offsets = arrow2::offset::Offsets::<i32>::try_from_lengths(
                                buffers.iter().map(|opt| opt.as_ref().map_or(0, |buf| buf.len()))
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

                            #quoted_declare_offsets

                            let #quoted_inner_data: Buffer<_> = #quoted_transparent_mapping;

                            #quoted_inner_bitmap

                            #quoted_create
                        }}
                    }
                }

                InnerRepr::NativeIterable => {
                    quote! {{
                        use arrow2::{buffer::Buffer, offset::OffsetsBuffer};

                        #quoted_declare_offsets

                        let #quoted_inner_data: Vec<_> =
                            #quoted_transparent_mapping
                            .collect();

                        #quoted_inner_bitmap

                        #quoted_create
                    }}
                }
            }
        }

        DataType::Struct(_) | DataType::Union(_, _, _) => {
            // NOTE: We always wrap objects with full extension metadata.
            let DataType::Extension(fqname, _, _) = datatype else {
                unreachable!()
            };
            let fqname_use = quote_fqname_as_type_path(fqname);
            let option_wrapper = if elements_are_nullable {
                quote! {}
            } else {
                quote! { .into_iter().map(Some) }
            };

            quote! {{
                _ = #bitmap_src;
                #fqname_use::to_arrow_opt(#data_src #option_wrapper)?
            }}
        }

        _ => unimplemented!("{datatype:#?}"),
    }
}
