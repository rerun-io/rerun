use crate::{DeserializationError, Loggable, SizeBytes};
use arrow2::{
    array::{StructArray, UInt64Array},
    datatypes::{DataType, Field},
};

use re_tuid::Tuid;

// ---

impl SizeBytes for Tuid {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        Tuid::heap_size_bytes(self)
    }
}

impl Loggable for Tuid {
    type Name = crate::ComponentName;

    #[inline]
    fn name() -> Self::Name {
        "rerun.controls.TUID".into()
    }

    #[inline]
    fn arrow_datatype() -> arrow2::datatypes::DataType {
        DataType::Struct(vec![
            Field::new("time_ns", DataType::UInt64, false),
            Field::new("inc", DataType::UInt64, false),
        ])
    }

    fn to_arrow_opt<'a>(
        _data: impl IntoIterator<Item = Option<impl Into<std::borrow::Cow<'a, Self>>>>,
    ) -> crate::SerializationResult<Box<dyn arrow2::array::Array>>
    where
        Self: 'a,
    {
        Err(crate::SerializationError::not_implemented(
            Self::name(),
            "TUIDs are never nullable, use `to_arrow()` instead",
        ))
    }

    #[inline]
    fn to_arrow<'a>(
        data: impl IntoIterator<Item = impl Into<std::borrow::Cow<'a, Self>>>,
    ) -> crate::SerializationResult<Box<dyn ::arrow2::array::Array>>
    where
        Self: 'a,
    {
        let (time_ns_values, inc_values): (Vec<_>, Vec<_>) = data
            .into_iter()
            .map(Into::into)
            .map(|tuid| (tuid.nanoseconds_since_epoch(), tuid.inc()))
            .unzip();

        let values = vec![
            UInt64Array::from_vec(time_ns_values).boxed(),
            UInt64Array::from_vec(inc_values).boxed(),
        ];
        let validity = None;

        // TODO(#3360): We use the extended type here because we rely on it for formatting.
        Ok(StructArray::new(Self::extended_arrow_datatype(), values, validity).boxed())
    }

    fn from_arrow(array: &dyn ::arrow2::array::Array) -> crate::DeserializationResult<Vec<Self>> {
        let expected_datatype = Self::arrow_datatype();
        let actual_datatype = array.data_type().to_logical_type();
        if actual_datatype != &expected_datatype {
            return Err(DeserializationError::datatype_mismatch(
                expected_datatype,
                actual_datatype.clone(),
            ));
        }

        // NOTE: Unwrap is safe everywhere below, datatype is checked above.
        // NOTE: We don't even look at the validity, our datatype says we don't care.

        let array = array.as_any().downcast_ref::<StructArray>().unwrap();

        // TODO(cmc): Can we rely on the fields ordering from the datatype? I would assume not
        // since we generally cannot rely on anything when it comes to arrow…
        // If we could, that would also impact our codegen deserialization path.
        let (time_ns_index, inc_index) = {
            let mut time_ns_index = None;
            let mut inc_index = None;
            for (i, field) in array.fields().iter().enumerate() {
                if field.name == "time_ns" {
                    time_ns_index = Some(i);
                } else if field.name == "inc" {
                    inc_index = Some(i);
                }
            }
            (time_ns_index.unwrap(), inc_index.unwrap())
        };

        let get_buffer = |field_index: usize| {
            array.values()[field_index]
                .as_any()
                .downcast_ref::<UInt64Array>()
                .unwrap()
                .values()
        };

        let time_ns_buffer = get_buffer(time_ns_index);
        let inc_buffer = get_buffer(inc_index);

        if time_ns_buffer.len() != inc_buffer.len() {
            return Err(DeserializationError::mismatched_struct_field_lengths(
                "time_ns",
                time_ns_buffer.len(),
                "inc",
                inc_buffer.len(),
            ));
        }

        Ok(time_ns_buffer
            .iter()
            .copied()
            .zip(inc_buffer.iter().copied())
            .map(|(time_ns, inc)| Self::from_nanos_and_inc(time_ns, inc))
            .collect())
    }
}

/// Implements [`crate::Component`] for any given type that is a simple wrapper
/// (newtype) around a [`Tuid`].
///
/// Usage:
/// ```ignore
/// re_types_core::delegate_arrow_tuid!(RowId);
/// ```
#[macro_export]
macro_rules! delegate_arrow_tuid {
    ($typ:ident as $fqname:expr) => {
        $crate::macros::impl_into_cow!($typ);

        impl $crate::Loggable for $typ {
            type Name = $crate::ComponentName;

            #[inline]
            fn name() -> Self::Name {
                $fqname.into()
            }

            #[inline]
            fn arrow_datatype() -> ::arrow2::datatypes::DataType {
                $crate::external::re_tuid::Tuid::arrow_datatype()
            }

            #[inline]
            fn to_arrow_opt<'a>(
                values: impl IntoIterator<Item = Option<impl Into<::std::borrow::Cow<'a, Self>>>>,
            ) -> $crate::SerializationResult<Box<dyn ::arrow2::array::Array>>
            where
                Self: 'a,
            {
                Err($crate::SerializationError::not_implemented(
                    Self::name(),
                    "TUIDs are never nullable, use `to_arrow()` instead",
                ))
            }

            #[inline]
            fn to_arrow<'a>(
                values: impl IntoIterator<Item = impl Into<std::borrow::Cow<'a, Self>>>,
            ) -> $crate::SerializationResult<Box<dyn ::arrow2::array::Array>> {
                let values = values.into_iter().map(|value| {
                    let value: ::std::borrow::Cow<'a, Self> = value.into();
                    value.into_owned()
                });
                <$crate::external::re_tuid::Tuid as $crate::Loggable>::to_arrow(
                    values.into_iter().map(|$typ(tuid)| tuid),
                )
            }

            #[inline]
            fn from_arrow(
                array: &dyn arrow2::array::Array,
            ) -> $crate::DeserializationResult<Vec<Self>> {
                Ok(
                    <$crate::external::re_tuid::Tuid as $crate::Loggable>::from_arrow(array)?
                        .into_iter()
                        .map(|tuid| Self(tuid))
                        .collect(),
                )
            }
        }
    };
}
