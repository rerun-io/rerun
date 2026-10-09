use arrow::datatypes::Schema;
use datafusion::sql::sqlparser::ast::Expr as SqlExpr;

use super::{ColumnFilter, leftmost_column, parse_sql_expr};

/// A filter of the table filter bar.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TableFilter {
    /// A filter the filter bar can edit.
    Column(ColumnFilter),

    /// Any other SQL expression.
    Custom(CustomFilter),
}

/// A filter as SQL the filter bar can't edit.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CustomFilter {
    pub sql: String,

    /// The leftmost column of the SQL, preferring columns of the schema over other identifiers.
    ///
    /// `None` if the SQL doesn't parse or has no identifiers.
    pub column: Option<String>,
}

impl CustomFilter {
    fn new(sql: &str, expr: Option<&SqlExpr>, schema: &Schema) -> Self {
        Self {
            sql: sql.to_owned(),
            column: expr.and_then(|expr| leftmost_column(expr, schema)),
        }
    }
}

impl TableFilter {
    /// Parse a filter from its SQL, given the schema of the table it filters.
    pub fn from_sql(sql: &str, schema: &Schema) -> Self {
        let expr = parse_sql_expr(sql).ok();
        expr.as_ref()
            .and_then(|expr| ColumnFilter::from_sql(expr, schema))
            .map_or_else(
                || Self::Custom(CustomFilter::new(sql, expr.as_ref(), schema)),
                Self::Column,
            )
    }

    /// The filter as SQL, or `None` if it selects every row.
    pub fn to_sql(&self) -> Option<String> {
        match self {
            Self::Column(column_filter) => column_filter.to_sql(),
            Self::Custom(custom_filter) => Some(custom_filter.sql.clone()),
        }
    }
}
