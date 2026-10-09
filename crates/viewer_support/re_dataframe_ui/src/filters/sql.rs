//! SQL form of table filters, as stored in the table blueprint.
//!
//! Each filter type writes one fixed shape of SQL per filter and parses that same shape back. Any
//! other SQL is kept as a [`TableFilter::Custom`] filter.

use std::ops::ControlFlow;

use arrow::datatypes::{DataType, Field, Schema};
use datafusion::common::{DFSchema, DataFusionError, plan_err};
use datafusion::execution::SessionState;
use datafusion::logical_expr::utils::{find_aggregate_exprs, find_window_exprs};
use datafusion::logical_expr::{Expr, ExprSchemable as _};
use datafusion::sql::parser::DFParser;
use datafusion::sql::sqlparser::ast::{
    BinaryOperator, Expr as SqlExpr, ExprWithAlias, FunctionArg, FunctionArgExpr,
    FunctionArguments, Ident, OneOrManyWithParens, UnaryOperator, Value, visit_expressions,
};

use super::ColumnFilter;
use super::sql_dialect::FilterDialect;

/// Name of the lambda parameter for the values of a list column.
const LAMBDA_PARAMETER: &str = "x";

/// Parse a SQL filter expression.
///
/// Supports lambdas such as `x -> x > 1`.
pub fn parse_sql_expr(sql: &str) -> Result<SqlExpr, DataFusionError> {
    DFParser::parse_sql_into_expr_with_dialect(sql, &FilterDialect::default()).map(|expr| expr.expr)
}

/// Plan a SQL filter for a dataframe with the given schema.
///
/// SQL that parses into a [`ColumnFilter`] runs as that filter. Any other SQL needs to return a
/// boolean.
pub fn filter_expression(
    state: &SessionState,
    schema: &DFSchema,
    sql: &str,
) -> Result<Expr, DataFusionError> {
    let sql_expr = parse_sql_expr(sql)?;

    if let Some(column_filter) = ColumnFilter::from_sql(&sql_expr, schema.as_arrow()) {
        return column_filter
            .as_filter_expression()
            .map_err(|err| DataFusionError::External(Box::new(err)));
    }

    let expr = state.create_logical_expr_from_sql_expr(
        ExprWithAlias {
            expr: sql_expr,
            alias: None,
        },
        schema,
    )?;

    if !find_aggregate_exprs([&expr]).is_empty() || !find_window_exprs([&expr]).is_empty() {
        return plan_err!("A filter can't use functions that combine rows, such as sum or count");
    }

    // A `NULL` predicate is a missing boolean, as in DataFusion.
    let data_type = expr.get_type(schema)?;
    let value_type = match &data_type {
        DataType::Dictionary(_, value_type) => value_type.as_ref(),
        data_type => data_type,
    };
    if !matches!(value_type, DataType::Boolean | DataType::Null) {
        return plan_err!("The filter produces {data_type} values instead of true or false");
    }

    Ok(expr)
}

/// The leftmost identifier of the expression that is a column of the schema, or else the leftmost
/// identifier.
///
/// For the SQL a filter type writes, this is the column it filters.
pub fn leftmost_column(expr: &SqlExpr, schema: &Schema) -> Option<String> {
    let mut first_ident = None;
    let column = visit_expressions(expr, |expr| {
        if let SqlExpr::Identifier(ident) = expr {
            let name = normalized_ident(ident);
            if schema.field_with_name(&name).is_ok() {
                return ControlFlow::Break(name);
            }
            first_ident.get_or_insert(name);
        }
        ControlFlow::Continue(())
    });
    match column {
        ControlFlow::Break(column) => Some(column),
        ControlFlow::Continue(()) => first_ident,
    }
}

/// The column a filter applies to.
pub struct SqlColumn<'a> {
    name: &'a str,
    is_list: bool,
}

impl<'a> SqlColumn<'a> {
    pub fn new(field: &'a Field) -> Self {
        Self {
            name: field.name(),
            is_list: matches!(field.data_type(), DataType::List(_) | DataType::ListView(_)),
        }
    }

    pub fn is_list(&self) -> bool {
        self.is_list
    }

    /// The quoted column name.
    pub fn sql(&self) -> String {
        quote_ident(self.name)
    }

    /// Build the SQL that selects rows where the predicate holds.
    ///
    /// `predicate` gets the SQL of one value. For a list column, a row is selected when the
    /// predicate holds for any of its values.
    pub fn predicate_sql(&self, predicate: impl FnOnce(&str) -> String) -> String {
        let column = self.sql();
        if self.is_list {
            format!(
                "any_match({column}, {LAMBDA_PARAMETER} -> {})",
                predicate(LAMBDA_PARAMETER)
            )
        } else {
            predicate(&column)
        }
    }

    /// The inverse of [`Self::predicate_sql`].
    ///
    /// Returns the predicate and the name of the value it applies to.
    pub fn match_predicate<'e>(&self, expr: &'e SqlExpr) -> Option<(&'e SqlExpr, SqlValue)> {
        if !self.is_list {
            return Some((
                strip_nested(expr),
                SqlValue {
                    name: self.name.to_owned(),
                },
            ));
        }

        let SqlExpr::Function(function) = strip_nested(expr) else {
            return None;
        };
        let function_name = function.name.to_string().to_lowercase();
        if !matches!(
            function_name.as_str(),
            "any_match" | "array_any_match" | "list_any_match"
        ) {
            return None;
        }

        let [column, lambda] = function_args(expr)?[..] else {
            return None;
        };
        if !(SqlValue {
            name: self.name.to_owned(),
        })
        .is(column)
        {
            return None;
        }

        let SqlExpr::Lambda(lambda) = lambda else {
            return None;
        };
        let parameter = match &lambda.params {
            OneOrManyWithParens::One(parameter) => parameter,
            OneOrManyWithParens::Many(parameters) => match &parameters[..] {
                [parameter] => parameter,
                _ => return None,
            },
        };

        Some((
            strip_nested(&lambda.body),
            SqlValue {
                name: normalized_ident(&parameter.name),
            },
        ))
    }
}

/// The value a predicate applies to, either a column or a lambda parameter.
pub struct SqlValue {
    name: String,
}

impl SqlValue {
    /// Is the expression this value?
    pub fn is(&self, expr: &SqlExpr) -> bool {
        matches!(strip_nested(expr), SqlExpr::Identifier(ident) if normalized_ident(ident) == self.name)
    }

    /// Match `value <op> right`, returning the operator and the right side.
    pub fn comparison<'e>(&self, expr: &'e SqlExpr) -> Option<(&'e BinaryOperator, &'e SqlExpr)> {
        match strip_nested(expr) {
            SqlExpr::BinaryOp { left, op, right } if self.is(left) => {
                Some((op, strip_nested(right)))
            }
            _ => None,
        }
    }
}

/// Negate a predicate, selecting the rows where it is false or null.
pub fn negated_sql(sql: &str) -> String {
    format!("({sql}) IS NOT TRUE")
}

/// The inverse of [`negated_sql`], returns whether the expression was negated.
pub fn strip_negation(expr: &SqlExpr) -> (&SqlExpr, bool) {
    match strip_nested(expr) {
        SqlExpr::IsNotTrue(inner) => (strip_nested(inner), true),
        expr => (expr, false),
    }
}

pub fn strip_nested(mut expr: &SqlExpr) -> &SqlExpr {
    while let SqlExpr::Nested(inner) = expr {
        expr = inner;
    }
    expr
}

pub fn quote_ident(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\"")) // NOLINT: SQL quoting, not Rust escaping
}

pub fn quote_string(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

/// Unquoted identifiers are case-insensitive, as in DataFusion.
fn normalized_ident(ident: &Ident) -> String {
    if ident.quote_style.is_some() {
        ident.value.clone()
    } else {
        ident.value.to_lowercase()
    }
}

pub fn string_literal(expr: &SqlExpr) -> Option<&str> {
    match strip_nested(expr) {
        SqlExpr::Value(value) => match &value.value {
            Value::SingleQuotedString(value) => Some(value),
            _ => None,
        },
        _ => None,
    }
}

pub fn bool_literal(expr: &SqlExpr) -> Option<bool> {
    match strip_nested(expr) {
        SqlExpr::Value(value) => match value.value {
            Value::Boolean(value) => Some(value),
            _ => None,
        },
        _ => None,
    }
}

/// The text of a number literal, including its sign.
pub fn number_literal(expr: &SqlExpr) -> Option<String> {
    match strip_nested(expr) {
        SqlExpr::Value(value) => match &value.value {
            Value::Number(number, _) => Some(number.clone()),
            _ => None,
        },
        SqlExpr::UnaryOp {
            op: UnaryOperator::Minus,
            expr,
        } => number_literal(expr).map(|number| format!("-{number}")),
        _ => None,
    }
}

/// Is the expression the given SQL, ignoring formatting?
pub fn is_same_sql(expr: &SqlExpr, sql: &str) -> bool {
    parse_sql_expr(sql).is_ok_and(|other| other.to_string() == strip_nested(expr).to_string())
}

/// The arguments of a function call without named or wildcard arguments.
fn function_args(expr: &SqlExpr) -> Option<Vec<&SqlExpr>> {
    let SqlExpr::Function(function) = strip_nested(expr) else {
        return None;
    };
    let FunctionArguments::List(list) = &function.args else {
        return None;
    };
    list.args
        .iter()
        .map(|arg| match arg {
            FunctionArg::Unnamed(FunctionArgExpr::Expr(expr)) => Some(expr),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use arrow::datatypes::{DataType, Field, Schema, TimeUnit};
    use strum::VariantArray as _;

    use super::*;
    use crate::filters::{
        ComparisonOperator, CustomFilter, FloatFilter, IntFilter, NonNullableBooleanFilter,
        NullableBooleanFilter, StringFilter, StringOperator, TableFilter, TimestampFilter,
        TypedFilter,
    };

    fn timestamp() -> jiff::Timestamp {
        jiff::Timestamp::from_millisecond(100_000_000_000).unwrap()
    }

    fn filters_for(data_type: &DataType, nullable: bool) -> Vec<TypedFilter> {
        let inner = match data_type {
            DataType::List(field) => field.data_type(),
            data_type => data_type,
        };

        match inner {
            DataType::Boolean if nullable => vec![
                NullableBooleanFilter::new_is_true().into(),
                NullableBooleanFilter::new_is_false().with_is_not().into(),
                NullableBooleanFilter::new_is_null().into(),
                NullableBooleanFilter::new_is_null().with_is_not().into(),
            ],
            DataType::Boolean => vec![
                NonNullableBooleanFilter::IsTrue.into(),
                NonNullableBooleanFilter::IsFalse.into(),
            ],
            DataType::Int64 => ComparisonOperator::VARIANTS
                .iter()
                .flat_map(|op| {
                    [
                        IntFilter::new(*op, Some(-42)).into(),
                        IntFilter::new(*op, Some(7)).into(),
                    ]
                })
                .collect(),
            DataType::Float64 => ComparisonOperator::VARIANTS
                .iter()
                .flat_map(|op| {
                    [
                        FloatFilter::new(*op, Some(-10.5)).into(),
                        FloatFilter::new(*op, Some(1e300)).into(),
                        FloatFilter::new(*op, Some(f64::INFINITY)).into(),
                    ]
                })
                .collect(),
            DataType::Utf8 => StringOperator::VARIANTS
                .iter()
                .flat_map(|op| {
                    [
                        StringFilter::new(*op, "query").into(),
                        StringFilter::new(*op, r"it's 100% a_b\c").into(),
                    ]
                })
                .collect(),
            DataType::Timestamp(_, _) => [
                TimestampFilter::today(),
                TimestampFilter::yesterday(),
                TimestampFilter::last_24_hours(),
                TimestampFilter::this_week(),
                TimestampFilter::last_week(),
                TimestampFilter::before(timestamp()),
                TimestampFilter::after(timestamp()),
                TimestampFilter::between(
                    timestamp(),
                    timestamp() + jiff::SignedDuration::from_hours(1),
                ),
            ]
            .into_iter()
            .flat_map(|filter| [filter.clone().into(), filter.with_is_not().into()])
            .collect(),
            _ => unreachable!(),
        }
    }

    /// Every filter type parses back from the SQL it writes, for plain and list columns.
    #[test]
    fn column_filters_round_trip_through_sql() {
        let data_types = [
            DataType::Boolean,
            DataType::Int64,
            DataType::Float64,
            DataType::Utf8,
            DataType::Timestamp(TimeUnit::Nanosecond, Some("UTC".into())),
        ];

        let mut all_sql = Vec::new();
        for data_type in data_types {
            for nullable in [false, true] {
                for is_list in [false, true] {
                    let column_type = if is_list {
                        DataType::new_list(data_type.clone(), nullable)
                    } else {
                        data_type.clone()
                    };
                    let field = Arc::new(Field::new("Some \"column\"", column_type, nullable));
                    let schema =
                        Schema::new_with_metadata(vec![Arc::clone(&field)], Default::default());

                    for filter in filters_for(&data_type, nullable) {
                        let column_filter = ColumnFilter::new(Arc::clone(&field), filter);
                        let sql = column_filter.to_sql().expect("filter selects some rows");
                        assert_eq!(
                            TableFilter::from_sql(&sql, &schema),
                            TableFilter::Column(column_filter),
                            "{sql}"
                        );
                        all_sql.push(sql);
                    }
                }
            }
        }

        insta::assert_snapshot!(all_sql.join("\n"));
    }

    /// Filters without a value select every row and have no SQL.
    #[test]
    fn empty_filters_have_no_sql() {
        let empty_filters: [(DataType, TypedFilter); 3] = [
            (
                DataType::Int64,
                IntFilter::new(ComparisonOperator::Eq, None).into(),
            ),
            (
                DataType::Float64,
                FloatFilter::new(ComparisonOperator::Eq, None).into(),
            ),
            (
                DataType::Utf8,
                StringFilter::new(StringOperator::Contains, "").into(),
            ),
        ];

        for (data_type, filter) in empty_filters {
            let field = Arc::new(Field::new("column", data_type, false));
            let filter = ColumnFilter::new(field, filter);
            assert_eq!(filter.to_sql(), None, "{filter:?}");
        }
    }

    /// SQL that no filter type writes is kept as written, together with its leftmost column.
    #[test]
    fn other_sql_is_custom() {
        let schema = Schema::new_with_metadata(
            vec![
                Field::new("a", DataType::Int64, false),
                Field::new("b", DataType::Int64, false),
            ],
            Default::default(),
        );

        for (sql, column) in [
            (r#""a" > "b""#, Some("a")),
            (r#""b" = "a""#, Some("b")),
            (r#""a" BETWEEN 1 AND 2"#, Some("a")),
            (r#"abs("b") > 1"#, Some("b")),
            (r#"any_match(x, x -> "b" > 1)"#, Some("b")),
            (r#""missing" > 1"#, Some("missing")),
            (r#""a" ILIKE '%x%'"#, Some("a")),
            ("1 = 1", None),
            ("not sql (", None),
        ] {
            assert_eq!(
                TableFilter::from_sql(sql, &schema),
                TableFilter::Custom(CustomFilter {
                    sql: sql.to_owned(),
                    column: column.map(str::to_owned),
                }),
                "{sql}"
            );
        }
    }

    /// SQL in the shape a filter type writes becomes a column filter, also with other casing and
    /// extra parentheses.
    #[test]
    fn hand_written_sql_is_parsed() {
        let field = Arc::new(Field::new("score", DataType::Int64, false));
        let schema = Schema::new_with_metadata(vec![Arc::clone(&field)], Default::default());
        assert_eq!(
            TableFilter::from_sql("(Score >= -3)", &schema),
            TableFilter::Column(ColumnFilter::new(
                field,
                IntFilter::new(ComparisonOperator::Ge, Some(-3))
            ))
        );
    }
}
