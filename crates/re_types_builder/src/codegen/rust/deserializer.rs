use arrow2::datatypes::DataType;
use convert_case::{Case, Casing as _};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use crate::{
    codegen::rust::{
        arrow::{quote_fqname_as_type_path, ArrowDataTypeTokenizer},
        util::is_tuple_struct_from_obj,
    },
    ArrowRegistry, Object, Objects,
};

// ---

/// The returned `TokenStream` always instantiates a `Vec<Option<T>>`.
///
/// This short-circuits on error using the `try` (`?`) operator: the outer scope must be one that
/// returns a `Result<_, DeserializationError>`!
pub fn quote_arrow_deserializer(
    arrow_registry: &ArrowRegistry,
    objects: &Objects,
    obj: &Object,
    data_src: &proc_macro2::Ident,
) -> TokenStream {
    let datatype = &arrow_registry.get(&obj.fqname);
    let quoted_datatype = ArrowDataTypeTokenizer(datatype, false);

    let obj_fqname = obj.fqname.as_str();
    let is_arrow_transparent = obj.datatype.is_none();
    let is_tuple_struct = is_tuple_struct_from_obj(obj);

    if is_arrow_transparent {
        // NOTE: Arrow transparent objects must have a single field, no more no less.
        // The semantic pass would have failed already if this wasn't the case.
        let obj_field = &obj.fields[0];
        let obj_field_fqname = obj_field.fqname.as_str();

        let data_src = data_src.clone();
        let data_dst = format_ident!(
            "{}",
            if is_tuple_struct {
                "data0"
            } else {
                obj_field.name.as_str()
            }
        );

        let quoted_deserializer = quote_arrow_field_deserializer(
            objects,
            &arrow_registry.get(&obj_field.fqname),
            obj_field.is_nullable,
            obj_field_fqname,
            &data_src,
        );

        let quoted_unwrapping = if obj_field.is_nullable {
            quote!(.map(Ok))
        } else {
            // error context is appended below
            quote!(.map(|v| v.ok_or_else(crate::DeserializationError::missing_data)))
        };

        let quoted_remapping = if is_tuple_struct {
            quote!(.map(|res| res.map(|v| Some(Self(v)))))
        } else {
            quote!(.map(|res| res.map(|#data_dst| Some(Self { #data_dst }))))
        };

        quote! {
            #quoted_deserializer
            #quoted_unwrapping
            #quoted_remapping
            // NOTE: implicit Vec<Result> to Result<Vec>
            .collect::<crate::DeserializationResult<Vec<Option<_>>>>()
            .with_context(#obj_field_fqname)?
        }
    } else {
        let data_src = data_src.clone();

        // NOTE: This can only be struct or union/enum at this point.
        match datatype.to_logical_type() {
            DataType::Struct(_) => {
                let data_src_fields = format_ident!("{data_src}_fields");
                let data_src_arrays = format_ident!("{data_src}_arrays");

                let quoted_field_deserializers = obj.fields.iter().map(|obj_field| {
                    let field_name = &obj_field.name;
                    let data_dst = format_ident!("{}", obj_field.name);

                    let quoted_deserializer = quote_arrow_field_deserializer(
                        objects,
                        &arrow_registry.get(&obj_field.fqname),
                        obj_field.is_nullable,
                        obj_field.fqname.as_str(),
                        &data_src,
                    );

                    quote! {
                        let #data_dst = {
                            // NOTE: `arrays_by_name` is a runtime collection of all of the input's
                            // payload's struct fields, while `#field_name` is the field we're
                            // looking for at comptime... there's no guarantee it's actually there at
                            // runtime!
                            if !arrays_by_name.contains_key(#field_name) {
                                return Err(crate::DeserializationError::missing_struct_field(
                                    #quoted_datatype, #field_name,
                                )).with_context(#obj_fqname);
                            }

                            // NOTE: The indexing by name is safe: checked above.
                            let #data_src = &**arrays_by_name[#field_name];
                             #quoted_deserializer
                        }
                    }
                });

                // NOTE: Collecting because we need it more than once.
                let quoted_field_names = obj
                    .fields
                    .iter()
                    .map(|field| format_ident!("{}", field.name))
                    .collect::<Vec<_>>();

                let quoted_unwrappings = obj.fields.iter().map(|obj_field| {
                    let obj_field_fqname = obj_field.fqname.as_str();
                    let quoted_obj_field_name = format_ident!("{}", obj_field.name);
                    if obj_field.is_nullable {
                        quote!(#quoted_obj_field_name)
                    } else {
                        quote! {
                            #quoted_obj_field_name: #quoted_obj_field_name
                                .ok_or_else(crate::DeserializationError::missing_data)
                                .with_context(#obj_field_fqname)?
                        }
                    }
                });

                let quoted_downcast = {
                    let cast_as = quote!(::arrow2::array::StructArray);
                    quote_array_downcast(obj_fqname, &data_src, cast_as, datatype)
                };
                quote! {{
                    let #data_src = #quoted_downcast?;
                    if #data_src.is_empty() {
                        // NOTE: The outer container is empty and so we already know that the end result
                        // is also going to be an empty vec.
                        // Early out right now rather than waste time computing possibly many empty
                        // datastructures for all of our children.
                        Vec::new()
                    } else {
                        let (#data_src_fields, #data_src_arrays) = (#data_src.fields(), #data_src.values());

                        let arrays_by_name: ::std::collections::HashMap<_, _> = #data_src_fields
                            .iter()
                            .map(|field| field.name.as_str())
                            .zip(#data_src_arrays)
                            .collect();

                        #(#quoted_field_deserializers;)*

                        arrow2::bitmap::utils::ZipValidity::new_with_validity(
                            ::itertools::izip!(#(#quoted_field_names),*),
                            #data_src.validity(),
                        )
                        .map(|opt| opt.map(|(#(#quoted_field_names),*)| Ok(Self { #(#quoted_unwrappings,)* })).transpose())
                        // NOTE: implicit Vec<Result> to Result<Vec>
                        .collect::<crate::DeserializationResult<Vec<_>>>()
                        .with_context(#obj_fqname)?
                    }
                }}
            }

            DataType::Union(_, _, arrow2::datatypes::UnionMode::Dense) => {
                let data_src_types = format_ident!("{data_src}_types");
                let data_src_arrays = format_ident!("{data_src}_arrays");
                let data_src_offsets = format_ident!("{data_src}_offsets");

                let quoted_field_deserializers =
                    obj.fields.iter().enumerate().map(|(i, obj_field)| {
                        let obj_field_fqname = &obj_field.fqname;
                        let data_dst = format_ident!("{}", obj_field.name.to_case(Case::Snake));

                        let quoted_deserializer = quote_arrow_field_deserializer(
                            objects,
                            &arrow_registry.get(&obj_field.fqname),
                            obj_field.is_nullable,
                            obj_field.fqname.as_str(),
                            &data_src,
                        );

                        let i = i + 1; // NOTE: +1 to account for `nulls` virtual arm

                        quote! {
                            let #data_dst = {
                                // NOTE: `data_src_arrays` is a runtime collection of all of the
                                // input's payload's union arms, while `#i` is our comptime union
                                // arm counter... there's no guarantee it's actually there at
                                // runtime!
                                if #i >= #data_src_arrays.len() {
                                    return Err(crate::DeserializationError::missing_union_arm(
                                        #quoted_datatype, #obj_field_fqname, #i,
                                    )).with_context(#obj_fqname);
                                }

                                // NOTE: The array indexing is safe: checked above.
                                let #data_src = &*#data_src_arrays[#i];
                                 #quoted_deserializer.collect::<Vec<_>>()
                            }
                        }
                    });

                let obj_fqname = obj.fqname.as_str();
                let quoted_obj_name = format_ident!("{}", obj.name);
                let quoted_branches = obj.fields.iter().enumerate().map(|(i, obj_field)| {
                    let i = i as i8 + 1; // NOTE: +1 to account for `nulls` virtual arm

                    let obj_field_fqname = obj_field.fqname.as_str();
                    let quoted_obj_field_name =
                        format_ident!("{}", obj_field.name.to_case(Case::Snake));
                    let quoted_obj_field_type =
                        format_ident!("{}", obj_field.name.to_case(Case::UpperCamel));

                    // TODO: uh-oh
                    let quoted_unwrap = if obj_field.is_nullable {
                        quote!()
                    } else {
                        quote!(.unwrap())
                    };

                    quote! {
                        #i => #quoted_obj_name::#quoted_obj_field_type({
                            // NOTE: It is absolutely crucial we explicitly handle the
                            // boundchecks manually first, otherwise rustc completely chokes
                            // when slicing the data (as in: a 100x perf drop)!
                            if offset as usize >= #quoted_obj_field_name.len() {
                                return Err(crate::DeserializationError::offsets_mismatch(
                                    (offset as _, offset as _), #quoted_obj_field_name.len()
                                )).with_context(#obj_field_fqname);
                            }

                            // Safety: all checked above.
                            #[allow(unsafe_code, clippy::undocumented_unsafe_blocks)]
                            unsafe { #quoted_obj_field_name.get_unchecked(offset as usize) }
                                .clone()
                                #quoted_unwrap
                        })
                    }
                });

                let quoted_downcast = {
                    let cast_as = quote!(::arrow2::array::UnionArray);
                    quote_array_downcast(obj_fqname, &data_src, &cast_as, datatype)
                };

                quote! {{
                    let #data_src = #quoted_downcast?;
                    if #data_src.is_empty() {
                        // NOTE: The outer container is empty and so we already know that the end result
                        // is also going to be an empty vec.
                        // Early out right now rather than waste time computing possibly many empty
                        // datastructures for all of our children.
                        Vec::new()
                    } else {
                        let (#data_src_types, #data_src_arrays) = (#data_src.types(), #data_src.fields());

                        let #data_src_offsets = #data_src.offsets()
                            .ok_or_else(|| crate::DeserializationError::datatype_mismatch(
                                #quoted_datatype, #data_src.data_type().clone(),
                            )).with_context(#obj_fqname)?;

                        if #data_src_types.len() > #data_src_offsets.len() {
                            return Err(crate::DeserializationError::offsets_mismatch(
                                (0, #data_src_types.len()), #data_src_offsets.len(),
                            )).with_context(#obj_fqname);
                        }

                        #(#quoted_field_deserializers;)*

                        #data_src_types
                            .iter()
                            .enumerate()
                            .map(|(i, typ)| {
                                // NOTE: Array indexing is safe, checked above.
                                let offset = #data_src_offsets[i];

                                if *typ == 0 {
                                    Ok(None)
                                } else {
                                    Ok(Some(match typ {
                                        #(#quoted_branches,)*
                                        _ => unreachable!(),
                                    }))
                                }
                            })
                            // NOTE: implicit Vec<Result> to Result<Vec>
                            .collect::<crate::DeserializationResult<Vec<_>>>()
                            .with_context(#obj_fqname)?
                    }
                }}
            }

            _ => unimplemented!("{datatype:#?}"),
        }
    }
}

/// The returned `TokenStream` always instantiates a `Vec<Option<T>>`.
///
/// This short-circuits on error using the `try` (`?`) operator: the outer scope must be one that
/// returns a `Result<_, DeserializationError>`!
fn quote_arrow_field_deserializer(
    objects: &Objects,
    datatype: &DataType,
    is_nullable: bool,
    obj_field_fqname: &str,
    data_src: &proc_macro2::Ident,
) -> TokenStream {
    _ = is_nullable; // not yet used, will be needed very soon

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
        | DataType::Float64
        | DataType::Boolean => {
            let quoted_unmapping =
                quote_iterator_unmapper(objects, datatype, IteratorKind::OptionValue, None);
            let quoted_unmapping = if *datatype.to_logical_type() == DataType::Boolean {
                quoted_unmapping
            } else {
                quote!(.map(|opt| opt.copied()) #quoted_unmapping)
            };

            let quoted_downcast = {
                let cast_as = format!("{:?}", datatype.to_logical_type()).replace("DataType::", "");
                let cast_as = format_ident!("{cast_as}Array");
                quote_array_downcast(obj_field_fqname, data_src, cast_as, datatype)
            };

            quote! {
                #quoted_downcast?
                    .into_iter() // NOTE: automatically checks the bitmap on our behalf
                    #quoted_unmapping
            }
        }

        DataType::Utf8 => {
            let quoted_downcast = {
                let cast_as = quote!(::arrow2::array::Utf8Array<i32>);
                quote_array_downcast(obj_field_fqname, data_src, cast_as, datatype)
            };

            let quoted_unmapping = quote_iterator_unmapper(
                objects,
                datatype,
                IteratorKind::ResultOptionValue,
                quote!(crate::ArrowString).into(),
            );

            let data_src_buf = format_ident!("{data_src}_buf");

            quote! {{
                let #data_src = #quoted_downcast?;
                let #data_src_buf = #data_src.values();

                let offsets = #data_src.offsets();
                arrow2::bitmap::utils::ZipValidity::new_with_validity(
                    offsets.iter().zip(offsets.lengths()),
                    #data_src.validity(),
                )
                .map(|elem| elem.map(|(start, len)| {
                        // NOTE: Do _not_ use `Buffer::sliced`, it panics on malformed inputs.

                        let start = *start as usize;
                        let end = start + len;

                        // NOTE: It is absolutely crucial we explicitly handle the
                        // boundchecks manually first, otherwise rustc completely chokes
                        // when slicing the data (as in: a 100x perf drop)!
                        if end as usize > #data_src_buf.len() {
                            return Err(crate::DeserializationError::offsets_mismatch(
                                (start, end), #data_src_buf.len(),
                            ));
                        }
                        // Safety: all checked above.
                        #[allow(unsafe_code, clippy::undocumented_unsafe_blocks)]
                        // NOTE: The `clone` is a `Buffer::clone`, which is just a refcount bump.
                        let data = unsafe { #data_src_buf.clone().sliced_unchecked(start, len) };

                        Ok(data)
                    }).transpose()
                )
                #quoted_unmapping
                // NOTE: implicit Vec<Result> to Result<Vec>
                .collect::<crate::DeserializationResult<Vec<Option<_>>>>()
                .with_context(#obj_field_fqname)?
                .into_iter()
            }}
        }

        DataType::FixedSizeList(inner, length) => {
            let data_src_inner = format_ident!("{data_src}_inner");
            let quoted_inner = quote_arrow_field_deserializer(
                objects,
                inner.data_type(),
                inner.is_nullable,
                obj_field_fqname,
                &data_src_inner,
            );

            let quoted_downcast = {
                let cast_as = quote!(::arrow2::array::FixedSizeListArray);
                quote_array_downcast(obj_field_fqname, data_src, cast_as, datatype)
            };

            let quoted_unmapping =
                quote_iterator_unmapper(objects, datatype, IteratorKind::ResultOptionValue, None);

            quote! {{
                let #data_src = #quoted_downcast?;
                if #data_src.is_empty() {
                    // NOTE: The outer container is empty and so we already know that the end result
                    // is also going to be an empty vec.
                    // Early out right now rather than waste time computing possibly many empty
                    // datastructures for all of our children.
                    Vec::new()
                } else {
                    let offsets = (0..).step_by(#length).zip((#length..).step_by(#length).take(#data_src.len()));

                    let #data_src_inner = {
                        let #data_src_inner = &**#data_src.values();
                        #quoted_inner.collect::<Vec<_>>()
                    };

                    arrow2::bitmap::utils::ZipValidity::new_with_validity(offsets, #data_src.validity())
                        .map(|elem| elem.map(|(start, end)| {
                                // NOTE: Do _not_ use `Buffer::sliced`, it panics on malformed inputs.

                                // We're manually generating our own offsets, then length must be correct.
                                debug_assert!(end - start == #length);

                                // NOTE: It is absolutely crucial we explicitly handle the
                                // boundchecks manually first, otherwise rustc completely chokes
                                // when slicing the data (as in: a 100x perf drop)!
                                if end as usize > #data_src_inner.len() {
                                    return Err(crate::DeserializationError::offsets_mismatch(
                                        (start, end), #data_src_inner.len(),
                                    ));
                                }
                                // Safety: all checked above.
                                #[allow(unsafe_code, clippy::undocumented_unsafe_blocks)]
                                let data = unsafe { #data_src_inner.get_unchecked(start as usize .. end as usize) };

                                // NOTE: The call to `Option::unwrap_or_default` is very important here.
                                //
                                // Since we can only get here if the outer entry is marked as
                                // non-null, the only possible reason for the default() path
                                // to be taken is because the inner field itself is nullable and
                                // happens to have one or more nullable values in the referenced
                                // slice.
                                // This is perfectly fine, and when it happens, we need to fill the
                                // resulting vec with some data, hence default().
                                //
                                // This does have a subtle implication though!
                                // Since we never even look at the inner field's data when the outer
                                // entry is null, it means we won't notice it if illegal/malformed/corrupt
                                // in any way.
                                // It is important that we turn a blind eye here, because most SDKs in
                                // the ecosystem will put illegal data (e.g. null entries in an array of
                                // non-null floats) in the inner buffer if the outer entry itself
                                // is null.
                                //
                                // TODO(#2875): use MaybeUninit rather than requiring a default impl
                                let data = data.iter().cloned().map(Option::unwrap_or_default);

                                // NOTE: Unwrapping cannot fail: the length must be correct.
                                let arr = array_init::from_iter(data).unwrap();

                                Ok(arr)
                            }).transpose()
                        )
                        #quoted_unmapping
                        // NOTE: implicit Vec<Result> to Result<Vec>
                        .collect::<crate::DeserializationResult<Vec<Option<_>>>>()?
                }
                .into_iter()
            }}
        }

        DataType::List(inner) => {
            let data_src_inner = format_ident!("{data_src}_inner");
            let quoted_inner = quote_arrow_field_deserializer(
                objects,
                inner.data_type(),
                inner.is_nullable,
                obj_field_fqname,
                &data_src_inner,
            );

            let quoted_downcast = {
                let cast_as = quote!(::arrow2::array::ListArray<i32>);
                quote_array_downcast(obj_field_fqname, data_src, cast_as, datatype)
            };

            quote! {{
                let #data_src = #quoted_downcast?;
                if #data_src.is_empty() {
                    // NOTE: The outer container is empty and so we already know that the end result
                    // is also going to be an empty vec.
                    // Early out right now rather than waste time computing possibly many empty
                    // datastructures for all of our children.
                    Vec::new()
                } else {
                    let #data_src_inner = {
                        let #data_src_inner = &**#data_src.values();
                        #quoted_inner.collect::<Vec<_>>()
                    };

                    let offsets = #data_src.offsets();
                    arrow2::bitmap::utils::ZipValidity::new_with_validity(
                        offsets.iter().zip(offsets.lengths()),
                        #data_src.validity(),
                    )
                    .map(|elem| elem.map(|(start, len)| {
                            // NOTE: Do _not_ use `Buffer::sliced`, it panics on malformed inputs.

                            let start = *start as usize;
                            let end = start + len;

                            // NOTE: It is absolutely crucial we explicitly handle the
                            // boundchecks manually first, otherwise rustc completely chokes
                            // when slicing the data (as in: a 100x perf drop)!
                            if end as usize > #data_src_inner.len() {
                                return Err(crate::DeserializationError::offsets_mismatch(
                                    (start, end), #data_src_inner.len(),
                                ));
                            }
                            // Safety: all checked above.
                            #[allow(unsafe_code, clippy::undocumented_unsafe_blocks)]
                            let data = unsafe { #data_src_inner.get_unchecked(start as usize .. end as usize) };

                            // NOTE: The call to `Option::unwrap_or_default` is very important here.
                            //
                            // Since we can only get here if the outer entry is marked as
                            // non-null, the only possible reason for the default() path
                            // to be taken is because the inner field itself is nullable and
                            // happens to have one or more nullable values in the referenced
                            // slice.
                            // This is perfectly fine, and when it happens, we need to fill the
                            // resulting vec with some data, hence default().
                            //
                            // This does have a subtle implication though!
                            // Since we never even look at the inner field's data when the outer
                            // entry is null, it means we won't notice it if illegal/malformed/corrupt
                            // in any way.
                            // It is important that we turn a blind eye here, because most SDKs in
                            // the ecosystem will put illegal data (e.g. null entries in an array of
                            // non-null floats) in the inner buffer if the outer entry itself
                            // is null.
                            //
                            // TODO(#2875): use MaybeUninit rather than requiring a default impl
                            let data = data.iter().cloned().map(Option::unwrap_or_default).collect();

                            Ok(data)
                        }).transpose()
                    )
                    // NOTE: implicit Vec<Result> to Result<Vec>
                    .collect::<crate::DeserializationResult<Vec<Option<_>>>>()?
                }
                .into_iter()
            }}
        }

        DataType::Struct(_) | DataType::Union(_, _, _) => {
            let DataType::Extension(fqname, _, _) = datatype else { unreachable!() };
            let fqname_use = quote_fqname_as_type_path(fqname);
            quote!(#fqname_use::try_from_arrow_opt(#data_src).with_context(#obj_field_fqname)?.into_iter())
        }

        _ => unimplemented!("{datatype:#?}"),
    }
}

/// Generates tokens that downcast the given array `arr` as `typ`, making sure to inject proper error handling.
fn quote_array_downcast(
    location: impl AsRef<str>,
    arr: &proc_macro2::Ident,
    cast_as: impl quote::ToTokens,
    expected_datatype: &DataType,
) -> TokenStream {
    let location = location.as_ref();
    let cast_as = cast_as.to_token_stream();
    let expected = ArrowDataTypeTokenizer(expected_datatype, false);
    quote! {
        #arr
            .as_any()
            .downcast_ref::<#cast_as>()
            .ok_or_else(|| crate::DeserializationError::datatype_mismatch(#expected, #arr.data_type().clone()))
            .with_context(#location)
    }
}

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
enum IteratorKind {
    ResultOptionValue,
    OptionResultValue,
    OptionValue,
    ResultValue,
    Value,
}

/// Generates code to unmap the data stuck within the depths of an iterator, no matter what.
#[allow(clippy::collapsible_else_if)]
fn quote_iterator_unmapper(
    objects: &Objects,
    datatype: &DataType,
    iter_kind: IteratorKind,
    extra_wrapper: Option<TokenStream>,
) -> TokenStream {
    let inner_obj = if let DataType::Extension(fqname, _, _) = datatype {
        Some(&objects[fqname])
    } else {
        None
    };
    let inner_is_arrow_transparent = inner_obj.map_or(false, |obj| obj.datatype.is_none());

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
            if let Some(extra_wrapper) = extra_wrapper {
                quote!(|v| #quoted_inner_obj_type(#extra_wrapper(v)))
            } else {
                quote!(|v| #quoted_inner_obj_type(v))
            }
        } else {
            if let Some(extra_wrapper) = extra_wrapper {
                quote!(|#quoted_data_dst| #quoted_inner_obj_type { #quoted_data_dst: #extra_wrapper(v) })
            } else {
                quote!(|#quoted_data_dst| #quoted_inner_obj_type { #quoted_data_dst })
            }
        };

        match iter_kind {
            IteratorKind::ResultOptionValue | IteratorKind::OptionResultValue => {
                quote!(.map(|res_or_opt| res_or_opt.map(|res_or_opt| res_or_opt.map(#quoted_binding))))
            }
            IteratorKind::OptionValue | IteratorKind::ResultValue => {
                quote!(.map(|res_or_opt| res_or_opt.map(#quoted_binding)))
            }
            IteratorKind::Value => quote!(.map(#quoted_binding)),
        }
    } else {
        if let Some(extra_wrapper) = extra_wrapper {
            let quoted_binding = quote!(|v| #extra_wrapper(v));
            match iter_kind {
                IteratorKind::ResultOptionValue | IteratorKind::OptionResultValue => {
                    quote!(.map(|res_or_opt| res_or_opt.map(|res_or_opt| res_or_opt.map(#quoted_binding))))
                }
                IteratorKind::OptionValue | IteratorKind::ResultValue => {
                    quote!(.map(|res_or_opt| res_or_opt.map(#quoted_binding)))
                }
                IteratorKind::Value => quote!(.map(#quoted_binding)),
            }
        } else {
            quote!()
        }
    }
}
