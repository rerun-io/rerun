use proc_macro2::{Literal, TokenStream};
use quote::{format_ident, quote};
use re_log::debug_assert;

use crate::codegen::rust::arrow::{
    ArrowDataTypeTokenizer, is_backed_by_scalar_buffer, quote_fqname_as_type_path,
};
use crate::codegen::rust::util::{is_tuple_struct_from_obj, quote_comment};
use crate::data_type::{AtomicDataType, DataType, UnionMode};
use crate::{Object, Objects, TypeRegistry};

// ---

/// This generates code that deserializes a runtime Arrow payload into the specified `obj`, taking
/// Arrow-transparency into account.
///
/// This short-circuits on error using the `try` (`?`) operator: the outer scope must be one that
/// returns a `Result<_, DeserializationError>`!
///
/// There is a 1:1 relationship between `quote_arrow_deserializer` and `Loggable::from_arrow_opt`:
/// ```ignore
/// fn from_arrow_opt(data: &dyn ::arrow::array::Array) -> DeserializationResult<Vec<Option<Self>>> {
///     Ok(#quoted_deserializer)
/// }
/// ```
///
/// This tells you two things:
/// - The runtime Arrow payload is always held in a variable `data`, identified as `data_src` below.
/// - The returned `TokenStream` must always instantiates a `Vec<Option<Self>>`.
///
/// ## Performance vs validation
/// The deserializers are designed for maximum performance, assuming the incoming data is correct.
/// If the data is not correct, the deserializers will return an error, but never panic or crash.
///
/// TODO(#5305): Currently we're doing a lot of checking for exact matches.
/// We should instead assume data is correct and handle errors gracefully.
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
    type_registry: &TypeRegistry,
    objects: &Objects,
    obj: &Object,
) -> TokenStream {
    // Runtime identifier of the variable holding the Arrow payload (`&dyn ::arrow::array::Array`).
    let data_src = format_ident!("arrow_data");

    let datatype = &type_registry.get(&obj.fqname);
    let quoted_self_datatype = quote! { Self::arrow_datatype() };

    let obj_fqname = obj.fqname.as_str();
    let is_enum = obj.is_enum();
    let is_arrow_transparent = obj.datatype.is_none();
    let is_tuple_struct = is_tuple_struct_from_obj(obj);

    if is_enum {
        // An enum is very similar to a transparent type.

        // As a transparent type, it's not clear what this does or
        // where it should come from. Also, it's not used in the internal
        // implementation of `quote_arrow_field_deserializer` anyways.
        // TODO(#6819): If we get rid of nullable components this will likely need to change.
        let is_nullable = true; // Will be ignored

        let obj_field_fqname = format!("{obj_fqname}#enum");

        let quoted_deserializer = quote_arrow_field_deserializer(
            objects,
            datatype.to_logical_type(),
            &quoted_self_datatype, // we are transparent, so the datatype of `Self` is the datatype of our contents
            is_nullable,
            &obj_field_fqname,
            &data_src,
            InnerRepr::NativeIterable,
        );

        let quoted_branches = obj.fields.iter().map(|obj_field| {
            let quoted_obj_field_type = format_ident!("{}", obj_field.name);

            // We should never hit this unwrap or it means the enum-processing at
            // the fbs layer is totally broken.
            let enum_value = obj_field.enum_or_union_variant_value.unwrap();
            let quoted_enum_value = proc_macro2::Literal::u64_unsuffixed(enum_value);

            quote! {
                Some(#quoted_enum_value) => Ok(Some(Self::#quoted_obj_field_type))
            }
        });

        // TODO(jleibs): We should be able to do this with try_from instead.
        let quoted_remapping = quote! {
            .map(|typ| {
                match typ {
                    // The actual enum variants
                    #(#quoted_branches,)*
                    None => Ok(None),
                    Some(invalid) => Err(DeserializationError::missing_union_arm(
                        #quoted_self_datatype, "<invalid>", invalid as _,
                    )),
                }
            })
        };

        quote! {
            #quoted_deserializer
            #quoted_remapping
            // NOTE: implicit Vec<Result> to Result<Vec>
            .collect::<DeserializationResult<Vec<Option<_>>>>()
            .with_context(#obj_fqname)?
        }
    } else if is_arrow_transparent {
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

        let field_datatype = type_registry.get(&obj_field.fqname);

        let quoted_deserializer = quote_arrow_field_deserializer(
            objects,
            &field_datatype,
            &quoted_self_datatype, // we are transparent, so the datatype of `Self` is the datatype of our contents
            obj_field.is_nullable,
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
                    let field_datatype = &type_registry.get(&obj_field.fqname);

                    let quoted_deserializer = quote_arrow_field_deserializer(
                        objects,
                        field_datatype,
                        &quote_datatype(field_datatype),
                        obj_field.is_nullable,
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
                                    #quoted_self_datatype, #field_name,
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
                    let cast_as = quote!(arrow::array::StructArray);
                    quote_array_downcast(obj_fqname, &data_src, cast_as, &quoted_self_datatype)
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
                        let (#data_src_fields, #data_src_arrays) = (#data_src.fields(), #data_src.columns());

                        let arrays_by_name: ::std::collections::HashMap<_, _> = #data_src_fields
                            .iter()
                            .map(|field| field.name().as_str())
                            .zip(#data_src_arrays)
                            .collect();

                        #(#quoted_field_deserializers;)*

                        ZipValidity::new_with_validity(
                            ::itertools::izip!(#(#quoted_field_names),*),
                            #data_src.nulls(),
                        )
                        .map(|opt| opt.map(|(#(#quoted_field_names),*)| Ok(Self { #(#quoted_unwrappings,)* })).transpose())
                        // NOTE: implicit Vec<Result> to Result<Vec>
                        .collect::<DeserializationResult<Vec<_>>>()
                        .with_context(#obj_fqname)?
                    }
                }}
            }

            DataType::Union(_, UnionMode::Sparse) => {
                // We use sparse arrow unions for c-style enums, which means only 8 bits is required for each field,
                // and nulls are encoded with a special 0-index `_null_markers` variant.

                let data_src_types = format_ident!("{data_src}_type_ids");

                let obj_fqname = obj.fqname.as_str();
                let quoted_branches = obj.fields.iter().enumerate().map(|(typ, obj_field)| {
                    let arrow_type_index = Literal::i8_unsuffixed(typ as i8 + 1); // 0 is reserved for `_null_markers`

                    let quoted_obj_field_type = format_ident!("{}", obj_field.name);
                    quote! {
                        #arrow_type_index => Ok(Some(Self::#quoted_obj_field_type))
                    }
                });

                let quoted_downcast = {
                    let cast_as = quote!(arrow::array::UnionArray);
                    quote_array_downcast(obj_fqname, &data_src, &cast_as, &quoted_self_datatype)
                };

                quote! {{
                    let #data_src = #quoted_downcast?;
                    let #data_src_types = #data_src.type_ids();

                    #data_src_types
                        .iter()
                        .map(|typ| {
                            match typ {
                                0 => Ok(None),

                                // The actual enum variants
                                #(#quoted_branches,)*

                                _ => Err(DeserializationError::missing_union_arm(
                                    #quoted_self_datatype, "<invalid>", *typ as _,
                                )),
                            }
                        })
                        // NOTE: implicit Vec<Result> to Result<Vec>
                        .collect::<DeserializationResult<Vec<_>>>()
                        .with_context(#obj_fqname)?
                }}
            }

            DataType::Union(_, UnionMode::Dense) => {
                // We use dense arrow unions for proper sum-type unions.
                // Nulls are encoded with a special 0-index `_null_markers` variant.

                let data_src_type_ids = format_ident!("{data_src}_type_ids");
                let data_src_offsets = format_ident!("{data_src}_offsets");

                let quoted_field_deserializers = obj
                    .fields
                    .iter()
                    .enumerate()
                    .filter(|(_, obj_field)| {
                        // For unit fields we don't have to collect any data.
                        obj_field.typ != crate::Type::Unit
                    })
                    .map(|(type_id, obj_field)| {
                        let data_dst = format_ident!("{}", obj_field.snake_case_name());

                        let field_datatype = &type_registry.get(&obj_field.fqname);
                        let quoted_deserializer = quote_arrow_field_deserializer(
                            objects,
                            field_datatype,
                            &quote_datatype(field_datatype),
                            obj_field.is_nullable,
                            obj_field.fqname.as_str(),
                            &data_src,
                            InnerRepr::NativeIterable,
                        );

                        let type_id = Literal::usize_unsuffixed(type_id + 1); // NOTE: +1 to account for `_null_markers` virtual arm

                        quote! {
                            let #data_dst = {
                                // `.child()` will panic if the given `type_id` doesn't exist,
                                // which could happen if the number of union arms has changed
                                // between serialization and deserialization.
                                // There is no simple way to check for this using `arrow-rs`
                                // (no access to `UnionArray::fields` as of arrow 54:
                                // https://docs.rs/arrow/latest/arrow/array/struct.UnionArray.html)
                                //
                                // Still, we're planning on removing arrow unions entirely, so this is… fine.
                                // TODO(#6388): stop using arrow unions, and remove this peril
                                let #data_src = #data_src.child(#type_id).as_ref();
                                #quoted_deserializer.collect::<Vec<_>>()
                            }
                        }
                    });

                let obj_fqname = obj.fqname.as_str();
                let quoted_branches = obj.fields.iter().enumerate().map(|(typ, obj_field)| {
                    let typ = typ as i8 + 1; // NOTE: +1 to account for `_null_markers` virtual arm

                    let obj_field_fqname = obj_field.fqname.as_str();
                    let quoted_obj_field_name = format_ident!("{}", obj_field.snake_case_name());
                    let quoted_obj_field_type = format_ident!("{}", obj_field.pascal_case_name());

                    if obj_field.typ == crate::Type::Unit {
                        // TODO(andreas): Should we check there's enough nulls on the null array?
                        return quote! {
                            #typ => Self::#quoted_obj_field_type
                        };
                    }

                    let quoted_unwrap = if obj_field.is_nullable {
                        quote!()
                    } else {
                        quote! {
                            .ok_or_else(DeserializationError::missing_data)
                            .with_context(#obj_field_fqname)?
                        }
                    };

                    quote! {
                        #typ => Self::#quoted_obj_field_type({
                            // NOTE: It is absolutely crucial we explicitly handle the
                            // boundchecks manually first, otherwise rustc completely chokes
                            // when indexing the data (as in: a 100x perf drop)!
                            if offset as usize >= #quoted_obj_field_name.len() {
                                return Err(DeserializationError::offset_oob(
                                    offset as _, #quoted_obj_field_name.len()
                                )).with_context(#obj_field_fqname);
                            }

                            // Safety: all checked above.
                            #[expect(unsafe_code, clippy::undocumented_unsafe_blocks)]
                            unsafe { #quoted_obj_field_name.get_unchecked(offset as usize) }
                                .clone()
                                #quoted_unwrap
                        })
                    }
                });

                let quoted_downcast = {
                    let cast_as = quote!(arrow::array::UnionArray);
                    quote_array_downcast(obj_fqname, &data_src, &cast_as, &quoted_self_datatype)
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
                        let #data_src_type_ids = #data_src.type_ids();

                        let #data_src_offsets = #data_src.offsets()
                            // NOTE: expected dense union, got a sparse one instead
                            .ok_or_else(|| {
                                let expected = #quoted_self_datatype;
                                let actual = #data_src.data_type().clone();
                                DeserializationError::datatype_mismatch(expected, actual)
                            }).with_context(#obj_fqname)?;

                        if #data_src_type_ids.len() != #data_src_offsets.len() {
                            // NOTE: need one offset array per union arm!
                            return Err(DeserializationError::offset_slice_oob(
                                (0, #data_src_type_ids.len()), #data_src_offsets.len(),
                            )).with_context(#obj_fqname);
                        }

                        #(#quoted_field_deserializers;)*

                        #data_src_type_ids
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
                                                #quoted_self_datatype, "<invalid>", *typ as _,
                                            ));
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
    /// The inner elements of the field should be exposed as `ScalarBuffer<T>`
    /// This is only applicable when T implements [`ArrowNativeType`](https://docs.rs/arrow/latest/arrow/datatypes/trait.ArrowNativeType.html).
    ScalarBuffer,

    /// The inner elements of the field should be exposed as an iterable of T
    NativeIterable,
}

/// This generates code that deserializes a runtime Arrow payload according to the specified `datatype`.
///
/// The `datatype` comes from our compile-time Arrow registry, not from the runtime payload!
/// If the datatype happens to be a struct or union, this will merely inject a runtime call to
/// `Loggable::from_arrow_opt` and call it a day, preventing code bloat.
///
/// `data_src` is the runtime identifier of the variable holding the Arrow payload (`&dyn ::arrow::array::Array`).
/// The returned `TokenStream` always instantiates a `Vec<Option<T>>`.
///
/// This short-circuits on error using the `try` (`?`) operator: the outer scope must be one that
/// returns a `Result<_, DeserializationError>`!
fn quote_arrow_field_deserializer(
    objects: &Objects,
    datatype: &DataType,
    quoted_datatype: &TokenStream,
    is_nullable: bool,
    obj_field_fqname: &str,
    data_src: &proc_macro2::Ident, // &dyn ::arrow::array::Array
    inner_repr: InnerRepr,
) -> TokenStream {
    _ = is_nullable; // not yet used, will be needed very soon

    // If the inner object is an enum, then dispatch to its deserializer.
    if let DataType::Object { fqname, .. } = datatype
        && objects.get(fqname).is_some_and(|obj| obj.is_enum())
    {
        let fqname_use = quote_fqname_as_type_path(fqname);
        return quote!(#fqname_use::from_arrow_opt(#data_src).with_context(#obj_field_fqname)?.into_iter());
    }

    match datatype.to_logical_type() {
        DataType::Atomic(atomic) => {
            let quoted_iter_transparency =
                quote_iterator_transparency(objects, datatype, IteratorKind::OptionValue, None);

            let quoted_downcast = {
                let cast_as = atomic.to_string();
                let cast_as = format_ident!("{cast_as}Array");
                quote_array_downcast(obj_field_fqname, data_src, cast_as, quoted_datatype)
            };

            match inner_repr {
                InnerRepr::ScalarBuffer => quote! {
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

        DataType::Binary => {
            // Special code to handle deserializing both 32-bit and 64-bit opffsets (BinaryArray vs LargeBinaryArray)
            quote! {{
                fn extract_from_binary<O>(
                    arrow_data: &arrow::array::GenericByteArray<arrow::datatypes::GenericBinaryType<O>>,
                ) -> DeserializationResult<std::vec::Vec<Option<arrow::buffer::Buffer>>>
                where
                    O: ::arrow::array::OffsetSizeTrait,
                {
                    use ::arrow::array::Array as _;
                    use ::re_types_core::arrow_zip_validity::ZipValidity;

                    let arrow_data_buf = arrow_data.values();
                    let offsets = arrow_data.offsets();

                    ZipValidity::new_with_validity(offsets.windows(2), arrow_data.nulls())
                        .map(|elem| {
                            elem.map(|window| {
                                // NOTE: Do _not_ use `Buffer::sliced`, it panics on malformed inputs.

                                let start = window[0].as_usize();
                                let end = window[1].as_usize();
                                let len = end - start;

                                // NOTE: It is absolutely crucial we explicitly handle the
                                // boundchecks manually first, otherwise rustc completely chokes
                                // when slicing the data (as in: a 100x perf drop)!
                                if arrow_data_buf.len() < end {
                                    // error context is appended below during final collection
                                    return Err(DeserializationError::offset_slice_oob(
                                        (start, end),
                                        arrow_data_buf.len(),
                                    ));
                                }

                                let data = arrow_data_buf.slice_with_length(start, len);
                                Ok(data)
                            })
                            .transpose()
                        })
                        .collect::<DeserializationResult<Vec<Option<_>>>>()
                }

                if let Some(arrow_data) = #data_src.as_any().downcast_ref::<BinaryArray>() {
                    extract_from_binary(arrow_data)
                        .with_context(#obj_field_fqname)?
                        .into_iter()
                } else if let Some(arrow_data) = #data_src.as_any().downcast_ref::<LargeBinaryArray>()
                {
                    extract_from_binary(arrow_data)
                        .with_context(#obj_field_fqname)?
                        .into_iter()
                } else {
                    let expected = Self::arrow_datatype();
                    let actual = arrow_data.data_type().clone();
                    return Err(DeserializationError::datatype_mismatch(expected, actual))
                        .with_context(#obj_field_fqname);
                }
            }}
        }

        DataType::Utf8 => {
            let quoted_downcast = {
                let cast_as = quote!(StringArray);
                quote_array_downcast(obj_field_fqname, data_src, cast_as, quoted_datatype)
            };

            let quoted_iter_transparency = quote_iterator_transparency(
                objects,
                datatype,
                IteratorKind::ResultOptionValue,
                quote!(::re_types_core::ArrowString::from).into(),
            );

            let data_src_buf = format_ident!("{data_src}_buf");

            quote! {{
                let #data_src = #quoted_downcast?;
                let #data_src_buf = #data_src.values();

                let offsets = #data_src.offsets();
                ZipValidity::new_with_validity(
                    offsets.windows(2),
                    #data_src.nulls(),
                )
                .map(|elem| elem.map(|window| {
                        // NOTE: Do _not_ use `Buffer::sliced`, it panics on malformed inputs.

                        let start = window[0] as usize;
                        let end = window[1] as usize;
                        let len = end - start;

                        // NOTE: It is absolutely crucial we explicitly handle the
                        // boundchecks manually first, otherwise rustc completely chokes
                        // when slicing the data (as in: a 100x perf drop)!
                        if #data_src_buf.len() < end {
                            // error context is appended below during final collection
                            return Err(DeserializationError::offset_slice_oob(
                                (start, end), #data_src_buf.len(),
                            ));
                        }
                        // TODO(apache/arrow-rs#6900): slice_with_length_unchecked unsafe when https://github.com/apache/arrow-rs/pull/6901 is merged and released
                        let data = #data_src_buf.slice_with_length(start, len);

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
                &quote_datatype(inner.data_type()),
                inner.is_nullable,
                obj_field_fqname,
                &data_src_inner,
                InnerRepr::NativeIterable,
            );

            let quoted_downcast = {
                let cast_as = quote!(arrow::array::FixedSizeListArray);
                quote_array_downcast(obj_field_fqname, data_src, cast_as, quoted_datatype)
            };

            let quoted_iter_transparency = quote_iterator_transparency(
                objects,
                datatype,
                IteratorKind::ResultOptionValue,
                None,
            );

            let comment_note_unwrap =
                quote_comment("NOTE: Unwrapping cannot fail: the length must be correct.");

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

                    ZipValidity::new_with_validity(offsets, #data_src.nulls())
                        .map(|elem| elem.map(|(start, end): (usize, usize)| {
                                // NOTE: Do _not_ use `Buffer::sliced`, it panics on malformed inputs.

                                // We're manually generating our own offsets in this case, thus length
                                // must be correct.
                                re_log::debug_assert!(end - start == #length);

                                // NOTE: It is absolutely crucial we explicitly handle the
                                // boundchecks manually first, otherwise rustc completely chokes
                                // when slicing the data (as in: a 100x perf drop)!
                                if #data_src_inner.len() < end {
                                    // error context is appended below during final collection
                                    return Err(DeserializationError::offset_slice_oob(
                                        (start, end), #data_src_inner.len(),
                                    ));
                                }
                                // Safety: all checked above.
                                #[expect(unsafe_code, clippy::undocumented_unsafe_blocks)]
                                let data = unsafe { #data_src_inner.get_unchecked(start..end) };

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

                                #comment_note_unwrap
                                #[expect(clippy::unwrap_used)]
                                Ok(array_init::from_iter(data).unwrap())
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

            let inner_repr = if is_backed_by_scalar_buffer(inner.data_type()) {
                InnerRepr::ScalarBuffer
            } else {
                InnerRepr::NativeIterable
            };

            let quoted_inner = quote_arrow_field_deserializer(
                objects,
                inner.data_type(),
                &quote_datatype(inner.data_type()),
                inner.is_nullable,
                obj_field_fqname,
                &data_src_inner,
                inner_repr,
            );

            let quoted_downcast = {
                let cast_as = quote!(arrow::array::ListArray);
                quote_array_downcast(obj_field_fqname, data_src, cast_as, quoted_datatype)
            };
            let quoted_collect_inner = match inner_repr {
                InnerRepr::ScalarBuffer => quote!(),
                InnerRepr::NativeIterable => quote!(.collect::<Vec<_>>()),
            };

            let quoted_inner_data_range = match inner_repr {
                InnerRepr::ScalarBuffer => {
                    quote! {
                        // TODO(apache/arrow-rs#6900): unsafe slice_unchecked when https://github.com/apache/arrow-rs/pull/6901 is merged and released
                        let data = #data_src_inner.clone().slice(start,  end - start);
                    }
                }
                InnerRepr::NativeIterable => quote! {
                    #[expect(unsafe_code, clippy::undocumented_unsafe_blocks)]
                    let data = unsafe { #data_src_inner.get_unchecked(start..end) };

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
                    ZipValidity::new_with_validity(
                        offsets.windows(2),
                        #data_src.nulls(),
                    )
                    .map(|elem| elem.map(|window| {
                            // NOTE: Do _not_ use `Buffer::sliced`, it panics on malformed inputs.

                            let start = window[0] as usize;
                            let end = window[1] as usize;

                            // NOTE: It is absolutely crucial we explicitly handle the
                            // boundchecks manually first, otherwise rustc completely chokes
                            // when slicing the data (as in: a 100x perf drop)!
                            if #data_src_inner.len() < end {
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

        DataType::Struct(_) | DataType::Union(_, _) => {
            let DataType::Object { fqname, .. } = datatype else {
                unreachable!()
            };
            let fqname_use = quote_fqname_as_type_path(fqname);
            quote!(#fqname_use::from_arrow_opt(#data_src).with_context(#obj_field_fqname)?.into_iter())
        }

        DataType::Object { .. } => unimplemented!("{datatype:#?}"),
    }
}

fn quote_datatype(datatype: &DataType) -> TokenStream {
    let expected = ArrowDataTypeTokenizer {
        datatype,
        recursive: false,
    };
    quote! { #expected }
}

/// Generates tokens that downcast the runtime Arrow array identifier by `arr` as `cast_as`, making sure
/// to inject proper error handling.
fn quote_array_downcast(
    location: impl AsRef<str>,
    arr: &syn::Ident,
    cast_as: impl quote::ToTokens,
    quoted_expected_datatype: &TokenStream,
) -> TokenStream {
    let location = location.as_ref();
    let cast_as = cast_as.to_token_stream();
    quote! {
        #arr
            .as_any()
            .downcast_ref::<#cast_as>()
            .ok_or_else(|| {
                let expected = #quoted_expected_datatype;
                let actual = #arr.data_type().clone();
                DeserializationError::datatype_mismatch(expected, actual)
            })
            .with_context(#location)
    }
}

#[derive(Debug, Clone, Copy)]
enum IteratorKind {
    /// `Iterator<Item = DeserializationResult<Option<T>>>`.
    ResultOptionValue,

    /// `Iterator<Item = Option<DeserializationResult<T>>>`.
    #[expect(dead_code)] // currently unused
    OptionResultValue,

    /// `Iterator<Item = Option<T>>`.
    OptionValue,

    /// `Iterator<Item = DeserializationResult<T>>`.
    #[expect(dead_code)] // currently unused
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
fn quote_iterator_transparency(
    objects: &Objects,
    datatype: &DataType,
    iter_kind: IteratorKind,
    extra_wrapper: Option<TokenStream>,
) -> TokenStream {
    #![expect(clippy::collapsible_else_if)]

    let inner_obj = if let DataType::Object { fqname, .. } = datatype {
        Some(&objects[fqname])
    } else {
        None
    };
    let inner_is_arrow_transparent = inner_obj.is_some_and(|obj| obj.datatype.is_none());

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
                quote!(#quoted_inner_obj_type)
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
/// fn from_arrow(data: &dyn ::arrow::array::Array) -> DeserializationResult<Vec<Self>> {
///     Ok(#quoted_deserializer_)
/// }
/// ```
///
/// See [`quote_arrow_deserializer_buffer_slice`] for additional information.
pub fn quote_arrow_deserializer_buffer_slice(
    type_registry: &TypeRegistry,
    objects: &Objects,
    obj: &Object,
) -> TokenStream {
    // Runtime identifier of the variable holding the Arrow payload (`&dyn ::arrow::array::Array`).
    let data_src = format_ident!("arrow_data");

    let datatype = &type_registry.get(&obj.fqname);

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

        let datatype = type_registry.get(&obj_field.fqname);
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
            quote!(.map(Self))
        } else {
            quote!(.map(|#data_dst| Self { #data_dst }))
        };

        quote! {{
            let slice = #deserizlized_as_slice;

            {
                // NOTE(#3850): Don't add a profile scope here: the profiler overhead is too big for this fast function.
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
    data_src: &proc_macro2::Ident, // &dyn ::arrow::array::Array
) -> TokenStream {
    _ = is_nullable; // not yet used, will be needed very soon

    match datatype.to_logical_type() {
        DataType::Atomic(atomic) => {
            let quoted_downcast = {
                let cast_as = atomic.to_string();
                let cast_as = format_ident!("{cast_as}Array"); // e.g. `Uint32Array`
                quote_array_downcast(
                    obj_field_fqname,
                    data_src,
                    cast_as,
                    &quote_datatype(datatype),
                )
            };

            quote! {
                #quoted_downcast?
                .values()
                .as_ref()
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
                let cast_as = quote!(arrow::array::FixedSizeListArray);
                quote_array_downcast(
                    obj_field_fqname,
                    data_src,
                    cast_as,
                    &quote_datatype(datatype),
                )
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
/// Note that nullabillity is kind of weird since it's technically a property of the field
/// rather than the datatype.
/// Components can only be used by archetypes so they should never be nullable, but for datatypes
/// we might need both.
///
/// This should always be checked before using [`quote_arrow_deserializer_buffer_slice`].
pub fn should_optimize_buffer_slice_deserialize(
    obj: &Object,
    type_registry: &TypeRegistry,
) -> bool {
    let is_arrow_transparent = obj.datatype.is_none();
    if is_arrow_transparent {
        let typ = type_registry.get(&obj.fqname);
        let obj_field = &obj.fields[0];
        !obj_field.is_nullable && should_optimize_buffer_slice_deserialize_datatype(&typ)
    } else {
        false
    }
}

/// Whether or not this datatype allows for the buffer slice optimizations.
fn should_optimize_buffer_slice_deserialize_datatype(typ: &DataType) -> bool {
    match typ {
        DataType::Atomic(atomic) => {
            !matches!(atomic, AtomicDataType::Null | AtomicDataType::Boolean)
        }
        DataType::Object { datatype, .. } => {
            should_optimize_buffer_slice_deserialize_datatype(datatype)
        }
        DataType::FixedSizeList(field, _) => {
            should_optimize_buffer_slice_deserialize_datatype(field.data_type())
        }
        _ => false,
    }
}
