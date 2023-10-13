use arrow2::{
    array::{Array, StructArray, UInt64Array},
    datatypes::{DataType, Field},
};

use crate::Tuid;

// ---

pub type DeserializationResult<T> = ::std::result::Result<T, DeserializationError>;

#[derive(Debug, thiserror::Error, Clone)]
pub enum DeserializationError {
    #[error("Expected {expected:#?} but found {got:#?} instead")]
    DatatypeMismatch {
        expected: ::arrow2::datatypes::DataType,
        got: ::arrow2::datatypes::DataType,
    },

    #[error("Expected field {field:#?} to be of unit-length but found {got} entries instead")]
    InvalidLength { field: String, got: usize },
}

impl Tuid {
    /// The underlying [`arrow2::datatypes::DataType`], excluding datatype extensions.
    #[inline]
    pub fn arrow_datatype() -> DataType {
        DataType::Struct(vec![
            Field::new("time_ns", DataType::UInt64, false),
            Field::new("inc", DataType::UInt64, false),
        ])
    }

    /// The underlying [`arrow2::datatypes::DataType`], including datatype extensions.
    #[inline]
    fn extended_arrow_datatype() -> arrow2::datatypes::DataType {
        DataType::Extension("rerun.tuid".into(), Box::new(Self::arrow_datatype()), None)
    }

    #[inline]
    pub fn as_arrow(&self) -> Box<dyn Array> {
        let extended_datatype = Self::extended_arrow_datatype();
        let values = vec![
            UInt64Array::from_vec(vec![self.time_ns]).boxed(),
            UInt64Array::from_vec(vec![self.inc]).boxed(),
        ];
        let validity = None;
        StructArray::new(extended_datatype, values, validity).boxed()
    }

    #[inline]
    pub fn from_arrow(array: &dyn Array) -> DeserializationResult<Self> {
        let expected_datatype = Self::arrow_datatype();
        let actual_datatype = array.data_type().to_logical_type();
        if actual_datatype != &expected_datatype {
            return Err(DeserializationError::DatatypeMismatch {
                expected: expected_datatype,
                got: actual_datatype.clone(),
            });
        }

        // NOTE: Unwrap is safe everywhere below, datatype is checked above.
        // NOTE: We don't even look at the validity, our datatype says we don't care.

        let array = array.as_any().downcast_ref::<StructArray>().unwrap();

        // TODO(cmc): Can we rely on the fields ordering from the datatype? I would assume not
        // since we generally cannot rely on anything when it comes to arrow...
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

        let get_first_value = |field_name: &str, field_index: usize| {
            let field_array = array.values()[field_index]
                .as_any()
                .downcast_ref::<UInt64Array>()
                .unwrap()
                .values();
            if field_array.len() != 1 {
                return Err(DeserializationError::InvalidLength {
                    field: field_name.into(),
                    got: field_array.len(),
                });
            }
            Ok(field_array[0])
        };

        Ok(Self {
            time_ns: get_first_value("time_ns", time_ns_index)?,
            inc: get_first_value("inc", inc_index)?,
        })
    }
}
