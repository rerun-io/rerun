use arrow2::datatypes::DataType;
use proc_macro2::{Literal, TokenStream};
use quote::quote;

// ---

/// `(Datatype, is_recursive)`
///
/// If `is_recursive` is set to `true`,
/// then the generated code will often be shorter, as it will
/// defer to calling `arrow_datatype()` on the inner type.
pub struct ArrowDataTypeTokenizer<'a>(pub &'a ::arrow2::datatypes::DataType, pub bool);

impl quote::ToTokens for ArrowDataTypeTokenizer<'_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        use arrow2::datatypes::UnionMode;
        let Self(datatype, recursive) = self;
        match datatype {
            DataType::Null => quote!(DataType::Null),
            DataType::Boolean => quote!(DataType::Boolean),
            DataType::Int8 => quote!(DataType::Int8),
            DataType::Int16 => quote!(DataType::Int16),
            DataType::Int32 => quote!(DataType::Int32),
            DataType::Int64 => quote!(DataType::Int64),
            DataType::UInt8 => quote!(DataType::UInt8),
            DataType::UInt16 => quote!(DataType::UInt16),
            DataType::UInt32 => quote!(DataType::UInt32),
            DataType::UInt64 => quote!(DataType::UInt64),
            DataType::Float16 => quote!(DataType::Float16),
            DataType::Float32 => quote!(DataType::Float32),
            DataType::Float64 => quote!(DataType::Float64),
            DataType::Binary => quote!(DataType::Binary),
            DataType::LargeBinary => quote!(DataType::LargeBinary),
            DataType::Utf8 => quote!(DataType::Utf8),
            DataType::LargeUtf8 => quote!(DataType::LargeUtf8),

            DataType::List(field) => {
                let field = ArrowFieldTokenizer::new(field);
                quote!(DataType::List(std::sync::Arc::new(#field)))
            }

            DataType::FixedSizeList(field, length) => {
                let field = ArrowFieldTokenizer::new(field);
                let length = Literal::usize_unsuffixed(*length);
                quote!(DataType::FixedSizeList(std::sync::Arc::new(#field), #length))
            }

            DataType::Union(fields, types, mode) => {
                let fields = fields.iter().map(ArrowFieldTokenizer::new);
                let mode = match mode {
                    UnionMode::Dense => quote!(UnionMode::Dense),
                    UnionMode::Sparse => quote!(UnionMode::Sparse),
                };
                if let Some(types) = types {
                    let types = types.iter().map(|&t| {
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
                } else {
                    quote!(DataType::Union(UnionFields::from(vec![ #(#fields,)* ]), #mode))
                }
            }

            DataType::Struct(fields) => {
                let fields = fields.iter().map(ArrowFieldTokenizer::new);
                quote!(DataType::Struct(Fields::from(vec![ #(#fields,)* ])))
            }

            DataType::Extension(fqname, datatype, _metadata) => {
                if *recursive {
                    // TODO(emilk): if the logical datatype is a primitive, then we can just use it directly
                    // so we get shorter generated code.
                    let fqname_use = quote_fqname_as_type_path(fqname);
                    quote!(<#fqname_use>::arrow_datatype())
                } else {
                    let datatype = ArrowDataTypeTokenizer(datatype.to_logical_type(), false);
                    quote!(#datatype)
                    // TODO(#3741): Bring back extensions once we've fully replaced `arrow2-convert`!
                    // let datatype = ArrowDataTypeTokenizer(datatype, false);
                    // let metadata = OptionTokenizer(metadata.as_ref());
                    // quote!(DataType::Extension(#fqname.to_owned(), Box::new(#datatype), #metadata))
                }
            }

            _ => unimplemented!("{:#?}", self.0),
        }
        .to_tokens(tokens);
    }
}

pub struct ArrowFieldTokenizer<'a> {
    field: &'a ::arrow2::datatypes::Field,
}

impl<'a> ArrowFieldTokenizer<'a> {
    pub fn new(field: &'a ::arrow2::datatypes::Field) -> Self {
        Self { field }
    }
}

impl quote::ToTokens for ArrowFieldTokenizer<'_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let Self { field } = self;
        let arrow2::datatypes::Field {
            name,
            data_type,
            is_nullable,
            metadata,
        } = field;

        // Unions in Rerun always has a `_null_markers` arm, so all unions are nullable,
        // whether they are specified as such or not.
        let is_nullable =
            *is_nullable || matches!(field.data_type.to_logical_type(), DataType::Union { .. });

        let datatype = ArrowDataTypeTokenizer(data_type, true);

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

// NOTE: Needed because `quote!()` interprets the option otherwise.
pub struct OptionTokenizer<T>(pub Option<T>);

impl<T: quote::ToTokens> quote::ToTokens for OptionTokenizer<T> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        if let Some(v) = &self.0 {
            quote!(Some(#v))
        } else {
            quote!(None)
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

pub fn is_backed_by_arrow_buffer(typ: &DataType) -> bool {
    matches!(
        typ,
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
    )
}

pub fn quoted_arrow_primitive_type(datatype: &DataType) -> TokenStream {
    match datatype {
        DataType::Null => quote!(NullType),
        DataType::Boolean => quote!(BooleanType),
        DataType::Int8 => quote!(Int8Type),
        DataType::Int16 => quote!(Int16Type),
        DataType::Int32 => quote!(Int32Type),
        DataType::Int64 => quote!(Int64Type),
        DataType::UInt8 => quote!(UInt8Type),
        DataType::UInt16 => quote!(UInt16Type),
        DataType::UInt32 => quote!(UInt32Type),
        DataType::UInt64 => quote!(UInt64Type),
        DataType::Float16 => quote!(Float16Type),
        DataType::Float32 => quote!(Float32Type),
        DataType::Float64 => quote!(Float64Type),
        _ => unimplemented!("Not a primitive type: {datatype:#?}"),
    }
}
