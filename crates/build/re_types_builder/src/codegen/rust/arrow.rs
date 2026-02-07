use proc_macro2::{Literal, TokenStream};
use quote::quote;

use crate::data_type::{AtomicDataType, DataType, Field, UnionMode};

// ---

pub struct ArrowDataTypeTokenizer<'a> {
    pub datatype: &'a DataType,

    /// If `true`,
    /// then the generated code will often be shorter, as it will
    /// defer to calling `arrow_datatype()` on the inner type.
    pub recursive: bool,
}

impl quote::ToTokens for ArrowDataTypeTokenizer<'_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let Self {
            datatype,
            recursive,
        } = self;
        match datatype {
            DataType::Atomic(AtomicDataType::Null) => quote!(DataType::Null),
            DataType::Atomic(AtomicDataType::Boolean) => quote!(DataType::Boolean),
            DataType::Atomic(AtomicDataType::Int8) => quote!(DataType::Int8),
            DataType::Atomic(AtomicDataType::Int16) => quote!(DataType::Int16),
            DataType::Atomic(AtomicDataType::Int32) => quote!(DataType::Int32),
            DataType::Atomic(AtomicDataType::Int64) => quote!(DataType::Int64),
            DataType::Atomic(AtomicDataType::UInt8) => quote!(DataType::UInt8),
            DataType::Atomic(AtomicDataType::UInt16) => quote!(DataType::UInt16),
            DataType::Atomic(AtomicDataType::UInt32) => quote!(DataType::UInt32),
            DataType::Atomic(AtomicDataType::UInt64) => quote!(DataType::UInt64),
            DataType::Atomic(AtomicDataType::Float16) => quote!(DataType::Float16),
            DataType::Atomic(AtomicDataType::Float32) => quote!(DataType::Float32),
            DataType::Atomic(AtomicDataType::Float64) => quote!(DataType::Float64),

            DataType::Binary => quote!(DataType::LargeBinary),

            DataType::Utf8 => quote!(DataType::Utf8),

            DataType::List(field) => {
                let field = ArrowFieldTokenizer::new(field);
                quote!(DataType::List(std::sync::Arc::new(#field)))
            }

            DataType::FixedSizeList(field, length) => {
                let field = ArrowFieldTokenizer::new(field);
                let length = Literal::usize_unsuffixed(*length);
                quote!(DataType::FixedSizeList(std::sync::Arc::new(#field), #length))
            }

            DataType::Union(fields, mode) => {
                let fields = fields.iter().map(ArrowFieldTokenizer::new);
                let mode = match mode {
                    UnionMode::Dense => quote!(UnionMode::Dense),
                    UnionMode::Sparse => quote!(UnionMode::Sparse),
                };

                let types = (0..fields.len()).map(|t| {
                    Literal::i8_unsuffixed(i8::try_from(t).unwrap_or_else(|_| {
                        panic!("Expect union type tag to be in 0-127; got {t}")
                    }))
                });
                quote!(DataType::Union(
                    UnionFields::new(
                        vec![ #(#types,)* ],
                        vec![ #(#fields,)* ],
                    ),
                    #mode,
                ))
            }

            DataType::Struct(fields) => {
                let fields = fields.iter().map(ArrowFieldTokenizer::new);
                quote!(DataType::Struct(Fields::from(vec![ #(#fields,)* ])))
            }

            DataType::Object { fqname, datatype } => {
                if *recursive {
                    // TODO(emilk): if the datatype is a primitive, then we can just use it directly
                    // so we get shorter generated code.
                    let fqname_use = quote_fqname_as_type_path(fqname);
                    quote!(<#fqname_use>::arrow_datatype())
                } else {
                    let datatype = ArrowDataTypeTokenizer {
                        datatype: datatype.to_logical_type(),
                        recursive: false,
                    };
                    quote!(#datatype)
                }
            }
        }
        .to_tokens(tokens);
    }
}

pub struct ArrowFieldTokenizer<'a> {
    field: &'a Field,
}

impl<'a> ArrowFieldTokenizer<'a> {
    pub fn new(field: &'a Field) -> Self {
        Self { field }
    }
}

impl quote::ToTokens for ArrowFieldTokenizer<'_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let Self { field } = self;
        let Field {
            name,
            data_type,
            is_nullable,
            metadata,
        } = field;

        // Unions in Rerun always has a `_null_markers` arm, so all unions are nullable,
        // whether they are specified as such or not.
        let is_nullable =
            *is_nullable || matches!(field.data_type.to_logical_type(), DataType::Union { .. });

        let datatype = ArrowDataTypeTokenizer {
            datatype: data_type,
            recursive: true,
        };

        let maybe_with_metadata = if metadata.is_empty() {
            quote!()
        } else {
            let metadata = StrStrMapTokenizer(metadata);
            quote!(.with_metadata(#metadata))
        };

        quote! {
            Field::new(#name, #datatype, #is_nullable)
            #maybe_with_metadata
        }
        .to_tokens(tokens);
    }
}

pub struct StrStrMapTokenizer<'a>(pub &'a std::collections::BTreeMap<String, String>);

impl quote::ToTokens for StrStrMapTokenizer<'_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let k = self.0.keys();
        let v = self.0.values();
        quote!([#((#k, #v),)*].into()).to_tokens(tokens);
    }
}

pub fn quote_fqname_as_type_path(fqname: impl AsRef<str>) -> TokenStream {
    let fqname = fqname.as_ref().replace('.', "::").replace("rerun", "crate");
    let expr: syn::TypePath = syn::parse_str(&fqname).unwrap();
    quote!(#expr)
}

/// Can this type be used with `arrow::ScalarBuffer`?
pub fn is_backed_by_scalar_buffer(typ: &DataType) -> bool {
    if let DataType::Atomic(atomic) = typ {
        !matches!(atomic, AtomicDataType::Null | AtomicDataType::Boolean)
    } else {
        false
    }
}

pub fn quoted_arrow_primitive_type(datatype: &DataType) -> TokenStream {
    match datatype {
        DataType::Atomic(AtomicDataType::Null) => quote!(NullType),
        DataType::Atomic(AtomicDataType::Boolean) => quote!(BooleanType),
        DataType::Atomic(AtomicDataType::Int8) => quote!(Int8Type),
        DataType::Atomic(AtomicDataType::Int16) => quote!(Int16Type),
        DataType::Atomic(AtomicDataType::Int32) => quote!(Int32Type),
        DataType::Atomic(AtomicDataType::Int64) => quote!(Int64Type),
        DataType::Atomic(AtomicDataType::UInt8) => quote!(UInt8Type),
        DataType::Atomic(AtomicDataType::UInt16) => quote!(UInt16Type),
        DataType::Atomic(AtomicDataType::UInt32) => quote!(UInt32Type),
        DataType::Atomic(AtomicDataType::UInt64) => quote!(UInt64Type),
        DataType::Atomic(AtomicDataType::Float16) => quote!(Float16Type),
        DataType::Atomic(AtomicDataType::Float32) => quote!(Float32Type),
        DataType::Atomic(AtomicDataType::Float64) => quote!(Float64Type),
        _ => unimplemented!("Not a primitive type: {datatype:#?}"),
    }
}
