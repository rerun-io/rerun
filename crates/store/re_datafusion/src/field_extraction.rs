use datafusion::arrow::array::{Array, ListArray, StructArray};
use datafusion::arrow::datatypes::{DataType, Field};
use datafusion::common::{plan_err, DataFusionError, ExprSchema, Result};
use datafusion::logical_expr::ScalarUDFImpl;
use datafusion::logical_expr::{ColumnarValue, Expr, Signature, Volatility};
use datafusion::scalar::ScalarValue;
use std::any::Any;
use std::sync::Arc;

#[derive(Debug)]
pub struct ExtractField {
    signature: Signature,
}

impl ExtractField {
    pub fn new() -> Self {
        Self {
            signature: Signature::any(2, Volatility::Immutable),
        }
    }
}

impl ScalarUDFImpl for ExtractField {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn name(&self) -> &str {
        "array_extract"
    }
    fn signature(&self) -> &Signature {
        &self.signature
    }

    fn return_type(&self, _args: &[DataType]) -> Result<DataType> {
        Err(DataFusionError::Internal(
            "Should have dispatched to return_type_from_exprs".to_owned(),
        ))
    }

    fn return_type_from_exprs(&self, args: &[Expr], schema: &dyn ExprSchema) -> Result<DataType> {
        let Some(Expr::Column(col)) = args.first() else {
            return plan_err!("rr_extract first arg must be a Column containing a List of Structs");
        };
        let dt = schema.data_type(col)?;

        let DataType::List(inner) = dt else {
            return plan_err!("rr_extract first arg must be a Column containing a List of Structs");
        };

        let DataType::Struct(fields) = inner.data_type() else {
            return plan_err!("rr_extract first arg must be a Column containing a List of Structs");
        };

        let Some(Expr::Literal(ScalarValue::Utf8(Some(field)))) = args.get(1) else {
            return plan_err!(
                "rr_extract second arg must be a string matching a field in the struct"
            );
        };

        let Some(final_dt) = fields.find(field) else {
            return plan_err!(
                "rr_extract second arg must be a string matching a field in the struct"
            );
        };

        Ok(DataType::List(Arc::new(Field::new(
            "item",
            final_dt.1.data_type().clone(),
            true,
        ))))
    }

    fn invoke(&self, args: &[ColumnarValue]) -> Result<ColumnarValue> {
        let ColumnarValue::Scalar(ScalarValue::Utf8(Some(field_name))) = &args[1] else {
            return Err(DataFusionError::Internal(
                "Expected second argument to be a string".to_owned(),
            ));
        };

        let args = ColumnarValue::values_to_arrays(args)?;
        let arg = &args[0];

        // Downcast to list array
        let Some(list_array) = arg.as_any().downcast_ref::<ListArray>() else {
            return Err(DataFusionError::Internal(
                "Expected first argument to be a ListArray".to_owned(),
            ));
        };

        // Get the child values array
        let child_values = list_array.values();

        // Downcast to a struct array
        let Some(struct_array) = child_values.as_any().downcast_ref::<StructArray>() else {
            return Err(DataFusionError::Internal(
                "Expected ListArray to contain StructArray".to_owned(),
            ));
        };

        // Get the values of the field with the correct name
        let Some(field_values) = struct_array.column_by_name(field_name) else {
            return Err(DataFusionError::Internal(format!(
                "Expected StructArray to contain field named '{field_name}'",
            )));
        };

        // Create a new list array with the same offsets but the child values
        let new_array = ListArray::new(
            Arc::new(Field::new("item", field_values.data_type().clone(), true)),
            list_array.offsets().clone(),
            field_values.clone(),
            list_array.nulls().cloned(),
        );

        Ok(ColumnarValue::Array(Arc::new(new_array)))
    }
}
