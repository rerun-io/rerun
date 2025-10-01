use std::any::Any;
use std::fmt::Debug;
use std::sync::{Arc, OnceLock};

use arrow::array::{ArrayRef, BooleanArray, ListArray, as_list_array};
use arrow::datatypes::DataType;
use datafusion::common::{Result as DataFusionResult, exec_err};
use datafusion::logical_expr::{
    ArrayFunctionArgument, ArrayFunctionSignature, ColumnarValue, ScalarFunctionArgs, ScalarUDF,
    ScalarUDFImpl, Signature, TypeSignature, Volatility,
};

pub trait FilterUdf: Any + Clone + Debug + Send + Sync {
    const PRIMITIVE_SIGNATURE: TypeSignature;

    fn name(&self) -> &'static str;
    fn is_valid_primitive_input_type(data_type: &DataType) -> bool;
    fn invoke_primitive_array(&self, array: &ArrayRef) -> DataFusionResult<BooleanArray>;

    fn as_scalar_udf(&self) -> ScalarUDF {
        ScalarUDF::new_from_impl(FilterUdfWrapper(self.clone()))
    }

    fn signature(&self) -> &Signature {
        static SIGNATURE: OnceLock<Signature> = OnceLock::new();

        SIGNATURE.get_or_init(|| {
            Signature::one_of(
                vec![
                    Self::PRIMITIVE_SIGNATURE,
                    TypeSignature::ArraySignature(ArrayFunctionSignature::Array {
                        arguments: vec![ArrayFunctionArgument::Array],
                        array_coercion: None,
                    }),
                ],
                Volatility::Immutable,
            )
        })
    }

    fn is_valid_input_type(data_type: &DataType) -> bool {
        match data_type {
            DataType::List(field) | DataType::ListView(field) => {
                // Note: we do not support double nested types
                Self::is_valid_primitive_input_type(field.data_type())
            }

            //TODO(ab): support other containers
            _ => Self::is_valid_primitive_input_type(data_type),
        }
    }

    fn invoke_list_array(&self, list_array: &ListArray) -> DataFusionResult<BooleanArray> {
        // TODO(ab): we probably should do this in two steps:
        // 1) Convert the list array to a bool array (with same offsets and nulls)
        // 2) Apply the ANY (or, in the future, another) semantics to "merge" each row's instances
        //    into the final bool.
        list_array
            .iter()
            .map(|maybe_row| {
                maybe_row.map(|row| {
                    // Note: we know this is a primitive array because we explicitly disallow nested
                    // lists or other containers.
                    let element_results = self.invoke_primitive_array(&row)?;

                    // `ANY` semantics happening here
                    Ok(element_results
                        .iter()
                        .map(|x| x.unwrap_or(false))
                        .find(|x| *x)
                        .unwrap_or(false))
                })
            })
            .map(|x| x.transpose())
            .collect::<DataFusionResult<BooleanArray>>()
    }
}

// shield against orphan rule
#[derive(Debug, Clone)]
struct FilterUdfWrapper<T: FilterUdf + Debug + Send + Sync>(T);

impl<T: FilterUdf + Debug + Send + Sync> ScalarUDFImpl for FilterUdfWrapper<T> {
    fn as_any(&self) -> &dyn Any {
        &self.0
    }

    fn name(&self) -> &'static str {
        self.0.name()
    }

    fn signature(&self) -> &Signature {
        self.0.signature()
    }

    fn return_type(&self, arg_types: &[DataType]) -> DataFusionResult<DataType> {
        if arg_types.len() != 1 {
            return exec_err!(
                "expected a single column of input, received {}",
                arg_types.len()
            );
        }

        if T::is_valid_input_type(&arg_types[0]) {
            Ok(DataType::Boolean)
        } else {
            exec_err!(
                "input data type {} not supported for {} filter UDF",
                arg_types[0],
                self.0.name()
            )
        }
    }

    fn invoke_with_args(&self, args: ScalarFunctionArgs) -> DataFusionResult<ColumnarValue> {
        let ColumnarValue::Array(input_array) = &args.args[0] else {
            return exec_err!("expected array inputs, not scalar values");
        };

        let results = match input_array.data_type() {
            DataType::List(_field) => {
                let array = as_list_array(input_array);
                self.0.invoke_list_array(array)?
            }

            //TODO(ab): support other containers
            data_type if T::is_valid_primitive_input_type(data_type) => {
                self.0.invoke_primitive_array(input_array)?
            }

            _ => {
                return exec_err!(
                    "DataType not implemented for {} filter UDF: {}",
                    self.0.name(),
                    input_array.data_type()
                );
            }
        };

        Ok(ColumnarValue::Array(Arc::new(results)))
    }
}
