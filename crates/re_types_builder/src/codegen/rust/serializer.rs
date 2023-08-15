use arrow2::datatypes::DataType;
use convert_case::{Case, Casing as _};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use crate::{ArrowRegistry, Object, Objects};

use super::{
    arrow::{is_backed_by_arrow_buffer, quote_fqname_as_type_path, ArrowDataTypeTokenizer},
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

    let DataType::Extension(fqname, _, _) = datatype else { unreachable!() };
    let fqname_use = quote_fqname_as_type_path(fqname);
    let quoted_datatype = quote! {
        (if let Some(ext) = extension_wrapper {
            DataType::Extension(ext.to_owned(), Box::new(<#fqname_use>::to_arrow_datatype()), None)
        } else {
            <#fqname_use>::to_arrow_datatype()
        })
        // TODO(cmc): Bring back extensions once we've fully replaced `arrow2-convert`!
        .to_logical_type().clone()
    };

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
            let #var: Option<::arrow2::bitmap::Bitmap> = {
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

        let quoted_serializer = quote_arrow_field_serializer(
            objects,
            Some(obj.fqname.as_str()),
            &arrow_registry.get(&obj_field.fqname),
            obj_field.is_nullable,
            &bitmap_dst,
            &quoted_data_dst,
            false,
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

                    let quoted_serializer = quote_arrow_field_serializer(
                        objects,
                        None,
                        &arrow_registry.get(&obj_field.fqname),
                        obj_field.is_nullable,
                        &bitmap_dst,
                        &data_dst,
                        false,
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

            DataType::Union(_, _, arrow2::datatypes::UnionMode::Dense) => {
                let quoted_field_serializers = obj.fields.iter().map(|obj_field| {
                    let data_dst = format_ident!("{}", obj_field.name.to_case(Case::Snake));
                    let bitmap_dst = format_ident!("{data_dst}_bitmap");

                    let quoted_serializer = quote_arrow_field_serializer(
                        objects,
                        None,
                        &arrow_registry.get(&obj_field.fqname),
                        obj_field.is_nullable,
                        &bitmap_dst,
                        &data_dst,
                        false
                    );

                    let quoted_flatten = quoted_flatten(obj_field.is_nullable);
                    let quoted_bitmap = quoted_bitmap(bitmap_dst);

                    let quoted_obj_name = format_ident!("{}", obj.name);
                    let quoted_obj_field_name = format_ident!("{}", obj_field.name.to_case(Case::UpperCamel));

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

                let quoted_types = {
                    let quoted_obj_name = format_ident!("{}", obj.name);
                    let quoted_branches = obj.fields.iter().enumerate().map(|(i, obj_field)| {
                        let i = i as i8 + 1; // NOTE: +1 to account for `nulls` virtual arm
                        let quoted_obj_field_name =
                            format_ident!("{}", obj_field.name.to_case(Case::UpperCamel));

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
                            format_ident!("{}_offset", obj_field.name.to_case(Case::Snake));
                        quote!(let mut #quoted_obj_field_name = 0)
                    });

                    let quoted_branches = obj.fields.iter().map(|obj_field| {
                        let quoted_counter_name =
                            format_ident!("{}_offset", obj_field.name.to_case(Case::Snake));
                        let quoted_obj_field_name =
                            format_ident!("{}", obj_field.name.to_case(Case::UpperCamel));
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
                    let #data_src: Vec<_> = #data_src
                        .into_iter()
                        .map(|datum| {
                            let datum: Option<::std::borrow::Cow<'a, Self>> = datum.map(Into::into);
                            datum
                        })
                        .collect();

                    UnionArray::new(
                        #quoted_datatype,
                        #quoted_types,
                        vec![
                            NullArray::new(
                                DataType::Null,
                                #data_src.iter().filter(|v| v.is_none()).count(),
                            ).boxed(),
                            #(#quoted_field_serializers,)*
                        ],
                        Some(#quoted_offsets),
                    ).boxed()
                }}
            }
            _ => unimplemented!("{datatype:#?}"),
        }
    }
}

fn quote_arrow_field_serializer(
    objects: &Objects,
    extension_wrapper: Option<&str>,
    datatype: &DataType,
    is_nullable: bool,
    bitmap_src: &proc_macro2::Ident,
    data_src: &proc_macro2::Ident,
    expose_inner_as_buffer: bool,
) -> TokenStream {
    let quoted_datatype = ArrowDataTypeTokenizer(datatype, false);
    let quoted_datatype = if let Some(ext) = extension_wrapper {
        quote!(DataType::Extension(#ext.to_owned(), Box::new(#quoted_datatype), None))
    } else {
        quote!(#quoted_datatype)
    };
    let quoted_datatype = quote! {{
        // NOTE: This is a field, it's never going to need the runtime one.
        _ = extension_wrapper;
        #quoted_datatype
            // TODO(cmc): Bring back extensions once we've fully replaced `arrow2-convert`!
            .to_logical_type().clone()
    }};

    let inner_obj = if let DataType::Extension(fqname, _, _) = datatype {
        Some(&objects[fqname])
    } else {
        None
    };
    let inner_is_arrow_transparent = inner_obj.map_or(false, |obj| obj.datatype.is_none());

    match datatype.to_logical_type() {
        DataType::Int8
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

            if expose_inner_as_buffer {
                // A primitive that's an inner element of a list will already have been mapped
                // to a buffer type.
                quote! {
                    PrimitiveArray::new(
                        #quoted_datatype,
                        #data_src,
                        #bitmap_src,
                    ).boxed()
                }
            } else {
                quote! {
                    PrimitiveArray::new(
                        #quoted_datatype,
                        #data_src.into_iter() #quoted_transparent_mapping .collect(),
                        #bitmap_src,
                    ).boxed()
                }
            }
        }

        DataType::Boolean => {
            quote! {
                BooleanArray::new(
                    #quoted_datatype,
                    // NOTE: We need values for all slots, regardless of what the bitmap says,
                    // hence `unwrap_or_default`.
                    #data_src.into_iter().map(|v| v.unwrap_or_default()).collect(),
                    #bitmap_src,
                ).boxed()
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
                let inner_data: ::arrow2::buffer::Buffer<u8> = #data_src.iter().flatten() #quoted_transparent_mapping.collect();

                let offsets = ::arrow2::offset::Offsets::<i32>::try_from_lengths(
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

            let expose_inner_as_buffer = is_backed_by_arrow_buffer(inner.data_type())
                && matches!(datatype, DataType::List(_));

            let quoted_inner_data = format_ident!("{data_src}_inner_data");
            let quoted_inner_bitmap = format_ident!("{data_src}_inner_bitmap");

            let quoted_inner = quote_arrow_field_serializer(
                objects,
                extension_wrapper,
                inner_datatype,
                inner.is_nullable,
                &quoted_inner_bitmap,
                &quoted_inner_data,
                expose_inner_as_buffer,
            );

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
            } else if expose_inner_as_buffer {
                quote! {
                    .flatten()
                    .map(|b| b.0.as_slice())
                    .collect::<Vec<_>>()
                    .concat()
                    .into();
                }
            } else {
                quote! {
                    .flatten()
                    // NOTE: Flattening yet again since we have to deconstruct the inner list.
                    .flatten()
                    .cloned()
                }
            };

            let quoted_num_instances = if expose_inner_as_buffer {
                quote!(num_instances())
            } else {
                quote!(len())
            };

            let quoted_create = if let DataType::List(_) = datatype {
                quote! {
                    let offsets = ::arrow2::offset::Offsets::<i32>::try_from_lengths(
                        #data_src.iter().map(|opt| opt.as_ref().map(|datum| datum. #quoted_num_instances).unwrap_or_default())
                    ).unwrap().into();

                    ListArray::new(
                        #quoted_datatype,
                        offsets,
                        #quoted_inner,
                        #bitmap_src,
                    ).boxed()
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

            // TODO(cmc): We should be checking this, but right now we don't because we don't
            // support intra-list nullability.
            _ = is_nullable;
            if expose_inner_as_buffer {
                quote! {{
                    use arrow2::{buffer::Buffer, offset::OffsetsBuffer};

                    let #quoted_inner_data: Buffer<_> = #data_src
                        .iter()
                        #quoted_transparent_mapping

                    // TODO(cmc): We don't support intra-list nullability in our IDL at the moment.
                    let #quoted_inner_bitmap: Option<::arrow2::bitmap::Bitmap> = None;

                    #quoted_create
                }}
            } else {
                quote! {{
                    use arrow2::{buffer::Buffer, offset::OffsetsBuffer};

                    let #quoted_inner_data: Vec<_> = #data_src
                        .iter()
                        #quoted_transparent_mapping
                        // NOTE: Wrapping back into an option as the recursive call will expect the
                        // guaranteed nullability layer to be present!
                        .map(Some)
                        .collect();

                    // TODO(cmc): We don't support intra-list nullability in our IDL at the moment.
                    let #quoted_inner_bitmap: Option<::arrow2::bitmap::Bitmap> = None;

                    #quoted_create
                }}
            }
        }

        DataType::Struct(_) | DataType::Union(_, _, _) => {
            // NOTE: We always wrap objects with full extension metadata.
            let DataType::Extension(fqname, _, _) = datatype else { unreachable!() };
            let fqname_use = quote_fqname_as_type_path(fqname);
            let quoted_extension_wrapper =
                extension_wrapper.map_or_else(|| quote!(None::<&str>), |ext| quote!(Some(#ext)));
            quote! {{
                _ = #bitmap_src;
                _ = extension_wrapper;
                #fqname_use::try_to_arrow_opt(#data_src, #quoted_extension_wrapper)?
            }}
        }

        _ => unimplemented!("{datatype:#?}"),
    }
}
