use arrow::{datatypes::Schema as ArrowSchema, error::ArrowError};

impl TryFrom<&crate::common::v1alpha1::Schema> for ArrowSchema {
    type Error = ArrowError;

    fn try_from(value: &crate::common::v1alpha1::Schema) -> Result<Self, Self::Error> {
        Ok(Self::clone(
            re_sorbet::schema_from_ipc(&value.arrow_schema)?.as_ref(),
        ))
    }
}

impl TryFrom<&ArrowSchema> for crate::common::v1alpha1::Schema {
    type Error = ArrowError;

    fn try_from(value: &ArrowSchema) -> Result<Self, Self::Error> {
        Ok(Self {
            arrow_schema: re_sorbet::ipc_from_schema(value)?,
        })
    }
}
