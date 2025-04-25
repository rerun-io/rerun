use arrow::datatypes::{DataType, Field};
use datafusion::common::{exec_err, Result as DataFusionResult};
use datafusion::logical_expr::{
    ColumnarValue, ReturnFieldArgs, ScalarFunctionArgs, ScalarUDFImpl, Signature, TypeSignature,
    Volatility,
};
use std::any::Any;

#[derive(Debug)]
pub struct SetEntityPathUdf {
    entity_path: String,
    signature: Signature,
}

impl SetEntityPathUdf {
    pub fn new(entity_path: impl Into<String>) -> Self {
        Self {
            entity_path: entity_path.into(),
            signature: Signature::new(TypeSignature::Any(1), Volatility::Immutable),
        }
    }
}

impl ScalarUDFImpl for SetEntityPathUdf {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn name(&self) -> &'static str {
        "set_entity_path"
    }

    fn signature(&self) -> &Signature {
        &self.signature
    }

    fn return_type(&self, _arg_types: &[DataType]) -> DataFusionResult<DataType> {
        exec_err!("use return_field_from_args instead")
    }

    fn return_field_from_args(&self, args: ReturnFieldArgs<'_>) -> DataFusionResult<Field> {
        if args.arg_fields.len() != 1 {
            exec_err!("UDF expects 1 argument")?;
        }

        let field = args
            .arg_fields
            .first()
            .expect("Incorrect number of arguments")
            .clone();
        let mut metadata = field.metadata().clone();
        metadata.insert("rerun.entity_path".to_owned(), self.entity_path.clone());

        Ok(field.with_metadata(metadata))
    }

    fn invoke_with_args(
        &self,
        args: ScalarFunctionArgs<'_, '_>,
    ) -> DataFusionResult<ColumnarValue> {
        Ok(args.args[0].clone())
    }
}
