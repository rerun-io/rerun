use std::any::Any;
use std::fmt::Debug;
use std::hash::Hash;
use std::sync::Arc;

use arrow::array::{ArrayRef, BooleanArray, ListArray, as_list_array};
use arrow::datatypes::DataType;
use datafusion::common::{Result as DataFusionResult, exec_err};
use datafusion::logical_expr::{
    ArrayFunctionArgument, ArrayFunctionSignature, ColumnarValue, ScalarFunctionArgs, ScalarUDF,
    ScalarUDFImpl, Signature, TypeSignature, Volatility,
};

/// Helper trait to make it straightforward to implement a filter UDF.
///
/// Note that a filter UDF is only a _building block_ towards creating a final expression for
/// datafusion. See [`super::Filter::as_filter_expression`] in its implementation for more details.
pub trait FilterUdf: Any + Clone + Debug + Send + Sync + Hash + PartialEq + Eq {
    /// The scalar datafusion type signature for this UDF.
    ///
    /// The list version will automatically be accepted as well, see `FilterUdfWrapper::signature`.
    const PRIMITIVE_SIGNATURE: TypeSignature;

    /// Name for this UDF.
    ///
    /// Keep it simple, it's also used in error. Example: "string" (for a string filter).
    fn name(&self) -> &'static str;

    /// Which _primitive_ datatypes are supported?
    ///
    /// Emphasis on "primitive". One layer of nested types (aka `List`) is automatically supported
    /// as well, see [`Self::is_valid_input_type`].
    fn is_valid_primitive_input_type(data_type: &DataType) -> bool;

    /// Invoke this UDF on a primitive array.
    ///
    /// Again, nested types (aka `List`) are automatically supported, see [`Self::invoke_list_array`].
    fn invoke_primitive_array(&self, array: &ArrayRef) -> DataFusionResult<BooleanArray>;

    /// Turn this type into a [`ScalarUDF`].
    fn as_scalar_udf(&self) -> ScalarUDF {
        ScalarUDF::new_from_impl(FilterUdfWrapper::new(self.clone()))
    }

    /// Is this datatype valid?
    ///
    /// Delegates to [`Self::is_valid_primitive_input_type`] for non-nested types.
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

    /// Invoke this UDF for a list array.
    ///
    /// Delegates actual implementation to [`Self::invoke_primitive_array`].
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

/// Wrapper for implementor of [`FilterUdf`].
///
/// This serves two purposes:
/// 1) Allow blanket implementation of [`ScalarUDFImpl`] (orphan rule)
/// 2) Cache the [`Signature`] (needed for [`ScalarUDFImpl::signature`])
#[derive(Debug, Eq, PartialEq, Hash, Clone)]
struct FilterUdfWrapper<T: FilterUdf> {
    inner: T,
    signature: Signature,
}

impl<T: FilterUdf> FilterUdfWrapper<T> {
    fn new(filter: T) -> Self {
        let signature = Signature::one_of(
            vec![
                T::PRIMITIVE_SIGNATURE,
                TypeSignature::ArraySignature(ArrayFunctionSignature::Array {
                    arguments: vec![ArrayFunctionArgument::Array],
                    array_coercion: None,
                }),
            ],
            Volatility::Immutable,
        );

        Self {
            inner: filter,
            signature,
        }
    }
}

impl<T: FilterUdf + Debug + Send + Sync> ScalarUDFImpl for FilterUdfWrapper<T> {
    fn as_any(&self) -> &dyn Any {
        &self.inner
    }

    fn name(&self) -> &'static str {
        self.inner.name()
    }

    fn signature(&self) -> &Signature {
        &self.signature
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
                self.inner.name()
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
                self.inner.invoke_list_array(array)?
            }

            //TODO(ab): support other containers
            data_type if T::is_valid_primitive_input_type(data_type) => {
                self.inner.invoke_primitive_array(input_array)?
            }

            _ => {
                return exec_err!(
                    "DataType not implemented for {} filter UDF: {}",
                    self.inner.name(),
                    input_array.data_type()
                );
            }
        };

        Ok(ColumnarValue::Array(Arc::new(results)))
    }
}
