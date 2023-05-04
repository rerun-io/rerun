use arrow2::{
    array::{BinaryArray, MutableBinaryArray, TryPush},
    datatypes::DataType,
};
use arrow2_convert::{deserialize::ArrowDeserialize, field::ArrowField, serialize::ArrowSerialize};

/// Helper for storing arbitrary serde-compatible types in an arrow2 [`BinaryArray`] field
///
/// Use as:
/// ```
/// use re_log_types::serde_field::SerdeField;
/// use arrow2_convert::{ArrowDeserialize, ArrowField, ArrowSerialize, field::ArrowField};
/// use arrow2::datatypes::{DataType, Field};
///
/// #[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
/// struct SomeStruct {
///     foo: String,
///     bar: u32,
/// }
///
/// #[derive(Clone, Debug, PartialEq, ArrowField, ArrowSerialize, ArrowDeserialize)]
/// #[arrow_field(transparent)]
/// struct SomeStructArrow(#[arrow_field(type = "SerdeField<SomeStruct>")] SomeStruct);
///
/// assert_eq!(SomeStructArrow::data_type(), DataType::Binary);
/// ```
pub struct SerdeField<T>(std::marker::PhantomData<T>);

impl<T> ArrowField for SerdeField<T>
where
    T: serde::ser::Serialize + serde::de::DeserializeOwned,
{
    type Type = T;

    #[inline]
    fn data_type() -> DataType {
        arrow2::datatypes::DataType::Binary
    }
}

impl<T> ArrowSerialize for SerdeField<T>
where
    T: serde::ser::Serialize + serde::de::DeserializeOwned,
{
    type MutableArrayType = MutableBinaryArray<i32>;
    #[inline]
    fn new_array() -> Self::MutableArrayType {
        MutableBinaryArray::new()
    }

    fn arrow_serialize(
        v: &<Self as ArrowField>::Type,
        array: &mut Self::MutableArrayType,
    ) -> arrow2::error::Result<()> {
        crate::profile_function!();
        let mut buf = Vec::new();
        rmp_serde::encode::write_named(&mut buf, v).map_err(|err| {
            arrow2::error::Error::ExternalFormat(format!("Could not encode as rmp: {err}"))
        })?;
        array.try_push(Some(buf))
    }
}

impl<T> ArrowDeserialize for SerdeField<T>
where
    T: serde::ser::Serialize + serde::de::DeserializeOwned,
{
    type ArrayType = BinaryArray<i32>;

    #[inline]
    fn arrow_deserialize(v: <&Self::ArrayType as IntoIterator>::Item) -> Option<T> {
        crate::profile_function!();
        v.and_then(|v| rmp_serde::from_slice::<T>(v).ok())
    }
}

#[cfg(test)]
mod tests {
    use super::SerdeField;
    use arrow2_convert::{ArrowDeserialize, ArrowField, ArrowSerialize};

    #[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
    struct SomeStruct {
        foo: String,
        bar: u32,
    }

    #[derive(Clone, Debug, PartialEq, ArrowField, ArrowSerialize, ArrowDeserialize)]
    struct SomeArrowStruct {
        #[arrow_field(type = "SerdeField<SomeStruct>")]
        field: SomeStruct,
    }

    impl From<SomeStruct> for SomeArrowStruct {
        fn from(s: SomeStruct) -> Self {
            Self { field: s }
        }
    }

    #[test]
    fn round_trip_serdefield() {
        use arrow2_convert::{deserialize::TryIntoCollection, serialize::TryIntoArrow};

        let data: [SomeArrowStruct; 2] = [
            SomeStruct {
                foo: "hello".into(),
                bar: 42,
            }
            .into(),
            SomeStruct {
                foo: "world".into(),
                bar: 1983,
            }
            .into(),
        ];
        let array: Box<dyn arrow2::array::Array> = data.try_into_arrow().unwrap();
        let ret: Vec<SomeArrowStruct> = array.try_into_collection().unwrap();
        assert_eq!(&data, ret.as_slice());
    }
}
