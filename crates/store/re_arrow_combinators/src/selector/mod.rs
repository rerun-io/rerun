//! Selector API for parsing and executing [`jq`](https://github.com/jqlang/jq/)-like queries on Arrow arrays.
//!
//! This module provides a high-level path-based API, but in contrast to `jq` its semantics are **columnar**,
//! following Apache Arrow's data model rather than a row-oriented object model.
//!
//! # Syntax
//!
//! The selector syntax is a subset of `jq`:
//!
//! | Syntax      | Meaning                                          | Example        |
//! |-------------|--------------------------------------------------|----------------|
//! | `.field`    | Access a named field in a struct                 | `.location`    |
//! | `[]`        | Iterate over every element of a list             | `.poses[]`     |
//! | `[N]`       | Index into a list by position                    | `.[0]`         |
//! | `?`         | Optional: suppress errors if a field is missing  | `.field?`      |
//! | `\|`        | Pipe the output of one expression to another     | `.foo \| .bar` |
//!
//! Segments can be chained without an explicit pipe: `.poses[].x` is equivalent to `.poses[] | .x`.
//!
//! # Differences from `jq`
//!
//! * **Columnar, not row-oriented** — operations apply to entire Arrow columns rather than individual JSON values.
//! * **No filters, arithmetic, or built-in functions** — only path navigation and iteration are supported.
//! * **No quoted field names or string interpolation** — field names must be bare identifiers
//!   (alphanumeric, `-`, `_`).

mod lexer;
mod parser;
mod runtime;

use arrow::{
    array::{Array as _, ListArray},
    datatypes::{DataType, Field},
};
use vec1::Vec1;

use parser::{Expr, Segment, SegmentKind};

/// A parsed selector expression that can be executed against Arrow arrays.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Selector(Expr);

impl std::fmt::Display for Selector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Selector {
    /// Execute this selector against each row of a [`ListArray`].
    ///
    /// Performs implicit iteration over the inner list array, and reconstructs the array at the end.
    ///
    /// `[.[].poses[].x]` is the actual query, we only require writing the `.poses[].x` portion.
    ///
    /// Returns `None` if the expression was suppressed by an optional segment (e.g. `.field?`).
    pub fn execute_per_row(&self, source: &ListArray) -> Result<Option<ListArray>, Error> {
        runtime::execute_per_row(&self.0, source).map_err(Into::into)
    }
}

impl std::str::FromStr for Selector {
    type Err = Error;

    fn from_str(query: &str) -> Result<Self, Self::Err> {
        // Lex the query string, collecting tokens and checking for lex errors
        let lexer = lexer::Lexer::new(query);
        let tokens = lexer.scan_tokens()?;

        let parser = parser::Parser::new(tokens.into_iter());
        let expr = parser.parse()?;

        Ok(Self(expr))
    }
}

impl crate::Transform for Selector {
    type Source = ListArray;
    type Target = ListArray;

    fn transform(&self, source: &Self::Source) -> Result<Self::Target, crate::Error> {
        let result = self.execute_per_row(source).map_err(crate::Error::from)?;
        Ok(result.unwrap_or_else(|| null_list_like(source)))
    }
}

impl crate::Transform for &Selector {
    type Source = ListArray;
    type Target = ListArray;

    fn transform(&self, source: &Self::Source) -> Result<Self::Target, crate::Error> {
        let result = self.execute_per_row(source).map_err(crate::Error::from)?;
        Ok(result.unwrap_or_else(|| null_list_like(source)))
    }
}

/// Creates an all-null [`ListArray`] with the same type and length as `source`.
fn null_list_like(source: &ListArray) -> ListArray {
    ListArray::new_null(
        Field::new_list_field(source.value_type(), true).into(),
        source.len(),
    )
}

/// Errors that can occur during selector parsing or execution.
#[derive(Debug, thiserror::Error, Clone)]
pub enum Error {
    /// Error during lexing.
    #[error(transparent)]
    Lex(#[from] lexer::Error),

    /// Error during parsing.
    #[error(transparent)]
    Parse(#[from] parser::Error),

    /// Error during runtime execution.
    #[error(transparent)]
    Runtime(#[from] crate::Error),
}

/// Extract nested fields from a struct array that match a predicate.
///
/// Returns `None` if no fields match the predicate, or if `datatype` is not a `DataType::Struct`.
pub fn extract_nested_fields<P>(
    datatype: &DataType,
    predicate: P,
) -> Option<Vec1<(Selector, DataType)>>
where
    P: Fn(&DataType) -> bool,
{
    let DataType::Struct(fields) = datatype else {
        return None;
    };

    let mut result = Vec::new();
    let mut queue = std::collections::VecDeque::new();

    // Initialize queue with root fields
    queue.push_back((Vec::new(), fields));

    // Breadth-first traversal
    while let Some((path, fields)) = queue.pop_front() {
        for field in fields {
            let mut field_path = path.clone();
            field_path.push(Segment {
                kind: SegmentKind::Field(field.name().clone()),
                optional: false,
            });

            match field.data_type() {
                DataType::Struct(nested_fields) => {
                    // Queue nested struct for later processing
                    queue.push_back((field_path, nested_fields));
                }
                DataType::List(inner) => {
                    // Add the Each segment to unwrap the list
                    field_path.push(Segment {
                        kind: SegmentKind::Each,
                        optional: false,
                    });

                    match inner.data_type() {
                        DataType::Struct(nested_fields) => {
                            // Queue nested struct within list for later processing
                            queue.push_back((field_path, nested_fields));
                        }
                        dt if predicate(dt) => {
                            // Direct match on list inner type
                            result.push((Selector(Expr::Path(field_path)), dt.clone()));
                        }
                        _ => {}
                    }
                }
                dt if predicate(dt) => {
                    // Direct match on field type
                    result.push((Selector(Expr::Path(field_path)), dt.clone()));
                }
                _ => {}
            }
        }
    }

    Vec1::try_from_vec(result).ok()
}
