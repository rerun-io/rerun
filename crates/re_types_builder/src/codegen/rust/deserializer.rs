use arrow2::datatypes::DataType;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use crate::{
    codegen::rust::{
        arrow::{is_backed_by_arrow_buffer, quote_fqname_as_type_path, ArrowDataTypeTokenizer},
        util::is_tuple_struct_from_obj,
    },
    ArrowRegistry, Object, ObjectField, ObjectKind, Objects,
};

// ---

/// This generates code that deserializes a runtime Arrow payload into the specified `obj`, taking
/// Arrow-transparency into account.
///
/// This short-circuits on error using the `try` (`?`) operator: the outer scope must be one that
/// returns a `Result<_, DeserializationError>`!
///
/// There is a 1:1 relationship between `quote_arrow_deserializer` and `Loggable::from_arrow_opt`:
/// ```ignore
/// fn from_arrow_opt(data: &dyn ::arrow2::array::Array) -> DeserializationResult<Vec<Option<Self>>> {
///     Ok(#quoted_deserializer)
/// }
/// ```
///
/// This tells you two things:
/// - The runtime Arrow payload is always held in a variable `data`, identified as `data_src` below.
/// - The returned `TokenStream` must always instantiates a `Vec<Option<Self>>`.
///
/// ## Understanding datatypes
///
/// There are three (!) datatypes involved in the deserialization process:
/// - The object's native Rust type, which was derived from its IDL definition by the codegen
///   framework.
/// - The object's Arrow datatype, which was also derived from its IDL definition.
/// - The runtime payload's advertised Arrow datatype.
///
/// The deserialization process is _entirely_ driven by our own compile-time IDL-derived definitions:
/// the runtime payload's advertised Arrow datatype is only ever used as a mean of checking whether
/// the data we receive can be coerced one way or another into something that fit our schema.
///
/// In some places that coercion can be very strict (if the data doesn't match exactly, we abort
/// with a runtime error) while in other it might be more relaxed for performance reasons
/// (e.g. ignore the fact that the data has a bitmap altogether).
pub fn quote_arrow_deserializer(
    arrow_registry: &ArrowRegistry,
    objects: &Objects,
    obj: &Object,
) -> TokenStream {
    // Runtime identifier of the variable holding the Arrow payload (`&dyn ::arrow2::array::Array`).
    let data_src = format_ident!("arrow_data");

    let datatype = &arrow_registry.get(&obj.fqname);
    let quoted_datatype = quote! { Self::arrow_datatype() };

    let obj_fqname = obj.fqname.as_str();
    let is_arrow_transparent = obj.datatype.is_none();
    let is_tuple_struct = is_tuple_struct_from_obj(obj);

    if is_arrow_transparent {
        // NOTE: Arrow transparent objects must have a single field, no more no less.
        // The semantic pass would have failed already if this wasn't the case.
        let obj_field = &obj.fields[0];
        let obj_field_fqname = obj_field.fqname.as_str();

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
            Some(obj_field),
            obj_field_fqname,
            &data_src,
            InnerRepr::NativeIterable,
        );

        let quoted_unwrapping = if obj_field.is_nullable {
            quote!(.map(Ok))
        } else {
            // error context is appended below during final collection
            quote!(.map(|v| v.ok_or_else(DeserializationError::missing_data)))
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
            .collect::<DeserializationResult<Vec<Option<_>>>>()
            // NOTE: double context so the user can see the transparent shenanigans going on in the
            // error.
            .with_context(#obj_field_fqname)
            .with_context(#obj_fqname)?
        }
    } else {
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
                        Some(obj_field),
                        obj_field.fqname.as_str(),
                        &data_src,
                        InnerRepr::NativeIterable,
                    );

                    quote! {
                        let #data_dst = {
                            // NOTE: `arrays_by_name` is a runtime collection of all of the input's
                            // payload's struct fields, while `#field_name` is the field we're
                            // looking for at comptime… there's no guarantee it's actually there at
                            // runtime!
                            if !arrays_by_name.contains_key(#field_name) {
                                return Err(DeserializationError::missing_struct_field(
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
                                .ok_or_else(DeserializationError::missing_data)
                                .with_context(#obj_field_fqname)?
                        }
                    }
                });

                let quoted_downcast = {
                    let cast_as = quote!(arrow2::array::StructArray);
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
                        .collect::<DeserializationResult<Vec<_>>>()
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
                        let data_dst = format_ident!("{}", obj_field.snake_case_name());

                        let quoted_deserializer = quote_arrow_field_deserializer(
                            objects,
                            &arrow_registry.get(&obj_field.fqname),
                            obj_field.is_nullable,
                            Some(obj_field),
                            obj_field.fqname.as_str(),
                            &data_src,
                            InnerRepr::NativeIterable,
                        );

                        let i = i + 1; // NOTE: +1 to account for `nulls` virtual arm

                        quote! {
                            let #data_dst = {
                                // NOTE: `data_src_arrays` is a runtime collection of all of the
                                // input's payload's union arms, while `#i` is our comptime union
                                // arm counter… there's no guarantee it's actually there at
                                // runtime!
                                if #i >= #data_src_arrays.len() {
                                    // By not returning an error but rather defaulting to an empty
                                    // vector, we introduce some kind of light forwards compatibility:
                                    // old clients that don't yet know about the new arms can still
                                    // send data in.
                                    return Ok(Vec::new());

                                    // return Err(DeserializationError::missing_union_arm(
                                    //     #quoted_datatype, #obj_field_fqname, #i,
                                    // )).with_context(#obj_fqname);
                                }

                                // NOTE: The array indexing is safe: checked above.
                                let #data_src = &*#data_src_arrays[#i];
                                 #quoted_deserializer.collect::<Vec<_>>()
                            }
                        }
                    });

                let obj_fqname = obj.fqname.as_str();
                let quoted_obj_name = format_ident!("{}", obj.name);
                let quoted_branches = obj.fields.iter().enumerate().map(|(typ, obj_field)| {
                    let typ = typ as i8 + 1; // NOTE: +1 to account for `nulls` virtual arm

                    let obj_field_fqname = obj_field.fqname.as_str();
                    let quoted_obj_field_name = format_ident!("{}", obj_field.snake_case_name());
                    let quoted_obj_field_type = format_ident!("{}", obj_field.pascal_case_name());

                    let quoted_unwrap = if obj_field.is_nullable {
                        quote!()
                    } else {
                        quote! {
                            .ok_or_else(DeserializationError::missing_data)
                            .with_context(#obj_field_fqname)?
                        }
                    };

                    quote! {
                        #typ => #quoted_obj_name::#quoted_obj_field_type({
                            // NOTE: It is absolutely crucial we explicitly handle the
                            // boundchecks manually first, otherwise rustc completely chokes
                            // when indexing the data (as in: a 100x perf drop)!
                            if offset as usize >= #quoted_obj_field_name.len() {
                                return Err(DeserializationError::offset_oob(
                                    offset as _, #quoted_obj_field_name.len()
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
                    let cast_as = quote!(arrow2::array::UnionArray);
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
                            // NOTE: expected dense union, got a sparse one instead
                            .ok_or_else(|| DeserializationError::datatype_mismatch(
                                #quoted_datatype, #data_src.data_type().clone(),
                            )).with_context(#obj_fqname)?;

                        if #data_src_types.len() != #data_src_offsets.len() {
                            // NOTE: need one offset array per union arm!
                            return Err(DeserializationError::offset_slice_oob(
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
                                        _ => {
                                            return Err(DeserializationError::missing_union_arm(
                                                #quoted_datatype, "<invalid>", *typ as _,
                                            )).with_context(#obj_fqname);
                                        }
                                    }))
                                }
                            })
                            // NOTE: implicit Vec<Result> to Result<Vec>
                            .collect::<DeserializationResult<Vec<_>>>()
                            .with_context(#obj_fqname)?
                    }
                }}
            }

            _ => unimplemented!("{datatype:#?}"),
        }
    }
}

#[derive(Copy, Clone, PartialEq)]
enum InnerRepr {
    /// The inner elements of the field should be exposed as `Buffer<T>`
    /// This is only applicable when T is an arrow primitive
    BufferT,

    /// The inner elements of the field should be exposed as an iterable of T
    NativeIterable,
}

/// This generates code that deserializes a runtime Arrow payload according to the specified `datatype`.
///
/// The `datatype` comes from our compile-time Arrow registry, not from the runtime payload!
/// If the datatype happens to be a struct or union, this will merely inject a runtime call to
/// `Loggable::from_arrow_opt` and call it a day, preventing code bloat.
///
/// `data_src` is the runtime identifier of the variable holding the Arrow payload (`&dyn ::arrow2::array::Array`).
/// The returned `TokenStream` always instantiates a `Vec<Option<T>>`.
///
/// This short-circuits on error using the `try` (`?`) operator: the outer scope must be one that
/// returns a `Result<_, DeserializationError>`!
fn quote_arrow_field_deserializer(
    objects: &Objects,
    datatype: &DataType,
    is_nullable: bool,
    obj_field: Option<&ObjectField>,
    obj_field_fqname: &str,
    data_src: &proc_macro2::Ident, // &dyn ::arrow2::array::Array
    inner_repr: InnerRepr,
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
            let quoted_iter_transparency =
                quote_iterator_transparency(objects, datatype, IteratorKind::OptionValue, None);
            let quoted_iter_transparency = if *datatype.to_logical_type() == DataType::Boolean {
                quoted_iter_transparency
            } else {
                quote!(.map(|opt| opt.copied()) #quoted_iter_transparency)
            };

            let quoted_downcast = {
                let cast_as = format!("{:?}", datatype.to_logical_type()).replace("DataType::", "");
                let cast_as = format_ident!("{cast_as}Array");
                quote_array_downcast(obj_field_fqname, data_src, cast_as, datatype)
            };

            match inner_repr {
                InnerRepr::BufferT => quote! {
                    #quoted_downcast?
                    .values()
                },
                InnerRepr::NativeIterable => quote! {
                    #quoted_downcast?
                        .into_iter() // NOTE: automatically checks the bitmap on our behalf
                        #quoted_iter_transparency
                },
            }
        }

        DataType::Utf8 => {
            let quoted_downcast = {
                let cast_as = quote!(arrow2::array::Utf8Array<i32>);
                quote_array_downcast(obj_field_fqname, data_src, cast_as, datatype)
            };

            let quoted_iter_transparency = quote_iterator_transparency(
                objects,
                datatype,
                IteratorKind::ResultOptionValue,
                quote!(::re_types_core::ArrowString).into(),
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
                            // error context is appended below during final collection
                            return Err(DeserializationError::offset_slice_oob(
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
                #quoted_iter_transparency
                // NOTE: implicit Vec<Result> to Result<Vec>
                .collect::<DeserializationResult<Vec<Option<_>>>>()
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
                None,
                obj_field_fqname,
                &data_src_inner,
                InnerRepr::NativeIterable,
            );

            let quoted_downcast = {
                let cast_as = quote!(arrow2::array::FixedSizeListArray);
                quote_array_downcast(obj_field_fqname, data_src, cast_as, datatype)
            };

            let quoted_iter_transparency = quote_iterator_transparency(
                objects,
                datatype,
                IteratorKind::ResultOptionValue,
                None,
            );

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

                                // We're manually generating our own offsets in this case, thus length
                                // must be correct.
                                debug_assert!(end - start == #length);

                                // NOTE: It is absolutely crucial we explicitly handle the
                                // boundchecks manually first, otherwise rustc completely chokes
                                // when slicing the data (as in: a 100x perf drop)!
                                if end as usize > #data_src_inner.len() {
                                    // error context is appended below during final collection
                                    return Err(DeserializationError::offset_slice_oob(
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
                                // The following would be the correct thing to do, but costs us way
                                // too much performance-wise for something that only applies to
                                // malformed inputs.
                                //
                                // // NOTE: We don't support nullable inner elements in our IDL, so
                                // // this can only be a case of malformed data.
                                // .map(|opt| opt.ok_or_else(DeserializationError::missing_data))
                                // .collect::<DeserializationResult<Vec<_>>>()?;

                                // NOTE: Unwrapping cannot fail: the length must be correct.
                                let arr = array_init::from_iter(data).unwrap();

                                Ok(arr)
                            }).transpose()
                        )
                        #quoted_iter_transparency
                        // NOTE: implicit Vec<Result> to Result<Vec>
                        .collect::<DeserializationResult<Vec<Option<_>>>>()?
                }
                .into_iter()
            }}
        }

        DataType::List(inner) => {
            let data_src_inner = format_ident!("{data_src}_inner");

            let inner_repr = if is_backed_by_arrow_buffer(inner.data_type()) {
                InnerRepr::BufferT
            } else {
                InnerRepr::NativeIterable
            };

            let quoted_inner = quote_arrow_field_deserializer(
                objects,
                inner.data_type(),
                inner.is_nullable,
                None,
                obj_field_fqname,
                &data_src_inner,
                inner_repr,
            );

            let quoted_downcast = {
                let cast_as = quote!(arrow2::array::ListArray<i32>);
                quote_array_downcast(obj_field_fqname, data_src, cast_as, datatype)
            };

            let serde_type = obj_field.and_then(|obj_field| {
                obj_field.try_get_attr::<String>(crate::ATTR_RUST_SERDE_TYPE)
            });

            let quoted_collect_inner = match inner_repr {
                InnerRepr::BufferT => quote!(),
                InnerRepr::NativeIterable => quote!(.collect::<Vec<_>>()),
            };

            let quoted_inner_data_range = match inner_repr {
                InnerRepr::BufferT => {
                    if let Some(serde_type) = serde_type.as_deref() {
                        let quoted_serde_type: syn::TypePath = syn::parse_str(serde_type).unwrap();
                        quote! {
                            #[allow(unsafe_code, clippy::undocumented_unsafe_blocks)]
                            let data = unsafe { #data_src_inner.clone().sliced_unchecked(start as usize,  end - start as usize) };
                            let data = rmp_serde::from_slice::<#quoted_serde_type>(data.as_slice()).map_err(|err| {
                                DeserializationError::serde_failure(err.to_string())
                            })?;
                        }
                    } else {
                        quote! {
                            #[allow(unsafe_code, clippy::undocumented_unsafe_blocks)]
                            let data = unsafe { #data_src_inner.clone().sliced_unchecked(start as usize,  end - start as usize) };
                            let data = ::re_types_core::ArrowBuffer::from(data);
                        }
                    }
                }
                InnerRepr::NativeIterable => quote! {
                    #[allow(unsafe_code, clippy::undocumented_unsafe_blocks)]
                    let data = unsafe { #data_src_inner.get_unchecked(start as usize .. end as usize) };

                    // NOTE: The call to `Option::unwrap_or_default` is very important here.
                    //
                    // Since we can only get here if the outer oob is marked as
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
                        // The following would be the correct thing to do, but costs us way
                        // too much performance-wise for something that only applies to
                        // malformed inputs.
                        //
                        // // NOTE: We don't support nullable inner elements in our IDL, so
                        // // this can only be a case of malformed data.
                        // .map(|opt| opt.ok_or_else(DeserializationError::missing_data))
                        // .collect::<DeserializationResult<Vec<_>>>()?;
                },
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
                        #quoted_inner #quoted_collect_inner
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
                                // error context is appended below during final collection
                                return Err(DeserializationError::offset_slice_oob(
                                    (start, end), #data_src_inner.len(),
                                ));
                            }

                            #quoted_inner_data_range

                            Ok(data)
                        }).transpose()
                    )
                    // NOTE: implicit Vec<Result> to Result<Vec>
                    .collect::<DeserializationResult<Vec<Option<_>>>>()?
                }
                .into_iter()
            }}
        }

        DataType::Struct(_) | DataType::Union(_, _, _) => {
            let DataType::Extension(fqname, _, _) = datatype else {
                unreachable!()
            };
            let fqname_use = quote_fqname_as_type_path(fqname);
            quote!(#fqname_use::from_arrow_opt(#data_src).with_context(#obj_field_fqname)?.into_iter())
        }

        _ => unimplemented!("{datatype:#?}"),
    }
}

/// Generates tokens that downcast the runtime Arrow array identifier by `arr` as `cast_as`, making sure
/// to inject proper error handling.
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
            .ok_or_else(|| DeserializationError::datatype_mismatch(#expected, #arr.data_type().clone()))
            .with_context(#location)
    }
}

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
enum IteratorKind {
    /// `Iterator<Item = DeserializationResult<Option<T>>>`.
    ResultOptionValue,

    /// `Iterator<Item = Option<DeserializationResult<T>>>`.
    OptionResultValue,

    /// `Iterator<Item = Option<T>>`.
    OptionValue,

    /// `Iterator<Item = DeserializationResult<T>>`.
    ResultValue,

    /// `Iterator<Item = T>`.
    Value,
}

/// This generates code that maps the data in an iterator in order to apply the Arrow transparency
/// rules to it, if necessary.
///
/// This can often become a very difficult job due to all the affixes that might be involved:
/// fallibility, nullability, transparency, tuple structs…
/// This function will just do the right thing.
///
/// If `extra_wrapper` is specified, this will also wrap the resulting data in `$extra_wrapper(data)`.
///
/// Have a look around in this file for examples of use.
#[allow(clippy::collapsible_else_if)]
fn quote_iterator_transparency(
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

/// This generates code that deserializes a runtime Arrow payload into the specified `obj`, taking
/// Arrow-transparency into account.
///
/// It contains additional performance optimizations based on the inner-type being a non-nullable primitive
/// allowing us to map directly to slices rather than iterating. The ability to use this optimization is
/// determined by [`should_optimize_buffer_slice_deserialize`].
///
/// There is a 1:1 relationship between `quote_arrow_deserializer_buffer_slice` and `Loggable::from_arrow`:
/// ```ignore
/// fn from_arrow(data: &dyn ::arrow2::array::Array) -> DeserializationResult<Vec<Self>> {
///     Ok(#quoted_deserializer_)
/// }
/// ```
///
/// See [`quote_arrow_deserializer_buffer_slice`] for additional information.
pub fn quote_arrow_deserializer_buffer_slice(
    arrow_registry: &ArrowRegistry,
    objects: &Objects,
    obj: &Object,
) -> TokenStream {
    // Runtime identifier of the variable holding the Arrow payload (`&dyn ::arrow2::array::Array`).
    let data_src = format_ident!("arrow_data");

    let datatype = &arrow_registry.get(&obj.fqname);

    let is_arrow_transparent = obj.datatype.is_none();
    let is_tuple_struct = is_tuple_struct_from_obj(obj);

    if is_arrow_transparent {
        // NOTE: Arrow transparent objects must have a single field, no more no less.
        // The semantic pass would have failed already if this wasn't the case.
        debug_assert!(obj.fields.len() == 1);
        let obj_field = &obj.fields[0];
        let obj_field_fqname = obj_field.fqname.as_str();

        let data_dst = format_ident!(
            "{}",
            if is_tuple_struct {
                "data0"
            } else {
                obj_field.name.as_str()
            }
        );

        let datatype = arrow_registry.get(&obj_field.fqname);
        let deserizlized_as_slice = quote_arrow_field_deserializer_buffer_slice(
            &datatype,
            obj_field.is_nullable,
            obj_field_fqname,
            &data_src,
        );

        let quoted_iter_transparency =
            quote_iterator_transparency(objects, &datatype, IteratorKind::Value, None);
        let quoted_iter_transparency = quote!(.copied() #quoted_iter_transparency);

        let quoted_remapping = if is_tuple_struct {
            quote!(.map(|v| Self(v)))
        } else {
            quote!(.map(|#data_dst| Self { #data_dst }))
        };

        quote! {{
            let slice = #deserizlized_as_slice;

            {
                // TODO(#3850): Don't, it's way too much and will therefore lie to you.
                // re_tracing::profile_scope!("collect");

                slice
                    .iter()
                    #quoted_iter_transparency
                    #quoted_remapping
                    .collect::<Vec<_>>()
            }
        }}
    } else {
        unimplemented!("{datatype:#?}")
    }
}

/// This generates code that deserializes a runtime Arrow payload according to the specified `datatype`.
///
/// It contains additional performance optimizations based on the inner-type being a non-nullable primitive
/// allowing us to map directly to slices rather than iterating. The ability to use this optimization is
/// determined by [`should_optimize_buffer_slice_deserialize`].
///
/// See [`quote_arrow_field_deserializer`] for additional information.
fn quote_arrow_field_deserializer_buffer_slice(
    datatype: &DataType,
    is_nullable: bool,
    obj_field_fqname: &str,
    data_src: &proc_macro2::Ident, // &dyn ::arrow2::array::Array
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
        | DataType::Float64 => {
            let quoted_downcast = {
                let cast_as = format!("{:?}", datatype.to_logical_type()).replace("DataType::", "");
                let cast_as = format_ident!("{cast_as}Array"); // e.g. `Uint32Array`
                quote_array_downcast(obj_field_fqname, data_src, cast_as, datatype)
            };

            quote! {
                #quoted_downcast?
                .values()
                .as_slice()
            }
        }

        DataType::FixedSizeList(inner, length) => {
            let data_src_inner = format_ident!("{data_src}_inner");
            let quoted_inner = quote_arrow_field_deserializer_buffer_slice(
                inner.data_type(),
                inner.is_nullable,
                obj_field_fqname,
                &data_src_inner,
            );

            let quoted_downcast = {
                let cast_as = quote!(arrow2::array::FixedSizeListArray);
                quote_array_downcast(obj_field_fqname, data_src, cast_as, datatype)
            };

            quote! {{
                let #data_src = #quoted_downcast?;

                let #data_src_inner = &**#data_src.values();
                bytemuck::cast_slice::<_, [_; #length]>(#quoted_inner)
            }}
        }

        _ => unimplemented!("{datatype:#?}"),
    }
}

/// Whether or not this object allows for the buffer-slice optimizations.
///
/// These optimizations require the outer type to be non-nullable and made up exclusively
/// of primitive types.
///
/// Nullabillity is kind of weird since it's technically a property of the field
/// rather than the datatype. However, we know that Components can only be used
/// by archetypes and as such should never be nullible.
///
/// This should always be checked before using [`quote_arrow_deserializer_buffer_slice`].
pub fn should_optimize_buffer_slice_deserialize(
    obj: &Object,
    arrow_registry: &ArrowRegistry,
) -> bool {
    let is_arrow_transparent = obj.datatype.is_none();
    if obj.kind == ObjectKind::Component && is_arrow_transparent {
        let typ = arrow_registry.get(&obj.fqname);
        let obj_field = &obj.fields[0];
        !obj_field.is_nullable && should_optimize_buffer_slice_deserialize_datatype(&typ)
    } else {
        false
    }
}

/// Whether or not this datatype allows for the buffer slice optimizations.
fn should_optimize_buffer_slice_deserialize_datatype(typ: &DataType) -> bool {
    match typ {
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
        | DataType::Float64 => true,
        DataType::Extension(_, typ, _) => should_optimize_buffer_slice_deserialize_datatype(typ),
        DataType::FixedSizeList(field, _) => {
            should_optimize_buffer_slice_deserialize_datatype(field.data_type())
        }
        _ => false,
    }
}
