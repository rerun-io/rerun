use arrow::datatypes::{DataType, Field};
use datafusion::common::DFSchema;
use datafusion::prelude::{Column, Expr, col, lit, regexp_like};

#[derive(Debug, Clone, thiserror::Error)]
pub enum FilterError {
    #[error("column {0} was not found")]
    ColumnNotFound(Column),

    #[error("invalid filter operation {0:?} for field {1}")]
    InvalidFilterOperation(FilterOperation, Box<Field>),
}

/// A filter applied to a table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Filter {
    pub column_name: String,
    pub operation: FilterOperation,
}

impl Filter {
    pub fn new(column_name: String, operation: FilterOperation) -> Self {
        Self {
            column_name,
            operation,
        }
    }

    /// Convert to an [`Expr`].
    ///
    /// The expression is used for filtering and should thus evaluate to a boolean.
    pub fn as_filter_expression(&self, schema: &DFSchema) -> Result<Expr, FilterError> {
        let column = Column::from(self.column_name.clone());
        let Ok(field) = schema.field_from_column(&column) else {
            return Err(FilterError::ColumnNotFound(column));
        };

        self.operation.as_filter_expression(&column, field)
    }
}

/// The kind of filter operation
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilterOperation {
    //TODO(ab): parameterise that over multiple string ops, e.g. "contains", "starts with", etc.
    StringContains(String),
}

impl FilterOperation {
    pub fn default_for_datatype(data_type: &DataType) -> Option<Self> {
        match data_type {
            DataType::Utf8 | DataType::Utf8View => Some(Self::StringContains(String::new())),

            DataType::List(field) | DataType::ListView(field)
                if field.data_type() == &DataType::Utf8
                    || field.data_type() == &DataType::Utf8View =>
            {
                Some(Self::StringContains(String::new()))
            }

            _ => None,
        }
    }

    /// Convert to an [`Expr`].
    ///
    /// The expression is used for filtering and should thus evaluate to a boolean.
    pub fn as_filter_expression(
        &self,
        column: &Column,
        field: &Field,
    ) -> Result<Expr, FilterError> {
        match self {
            Self::StringContains(query_string) => {
                if query_string.is_empty() {
                    return Ok(lit(true));
                }

                let operand = match field.data_type() {
                    DataType::Utf8 | DataType::Utf8View => col(column.clone()),

                    DataType::List(field) | DataType::ListView(field)
                        if field.data_type() == &DataType::Utf8
                            || field.data_type() == &DataType::Utf8View =>
                    {
                        // for List[Utf8], we concatenate all the instances into a single logical
                        // string
                        datafusion::prelude::array_to_string(col(column.clone()), lit(" "))
                    }

                    _ => {
                        return Err(FilterError::InvalidFilterOperation(
                            self.clone(),
                            field.clone().into(),
                        ));
                    }
                };

                Ok(regexp_like(operand, lit(query_string), Some(lit("i"))))
            }
        }
    }
}
