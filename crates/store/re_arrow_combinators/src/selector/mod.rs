//! Selector API for parsing and executing [`jq`](https://github.com/jqlang/jq/)-like queries on Arrow arrays.
//!
//! This module provides a high-level path-based API, but in contrast to jq its semantics are **columnar**,
//! following Apache Arrow's data model rather than a row-oriented object model.

// TODO(RR-3409): Explain the syntax and the similarities/differences to `jq` in the documentation.

mod lexer;
mod parser;
mod runtime;

use arrow::{
    array::ListArray,
    datatypes::{DataType, Fields},
};
use vec1::Vec1;

use parser::{Expr, Segment};

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
    pub fn execute_per_row(&self, source: &ListArray) -> Result<ListArray, Error> {
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
        self.execute_per_row(source).map_err(Into::into)
    }
}

impl crate::Transform for &Selector {
    type Source = ListArray;
    type Target = ListArray;

    fn transform(&self, source: &Self::Source) -> Result<Self::Target, crate::Error> {
        self.execute_per_row(source).map_err(Into::into)
    }
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
    fn rec<P>(acc: &mut Vec<(Selector, DataType)>, curr: &[Segment], fields: &Fields, predicate: &P)
    where
        P: Fn(&DataType) -> bool,
    {
        // First pass: collect all matching fields at the current level (breadth-first)
        for field in fields {
            let mut path = curr.to_vec();
            path.push(Segment::Field(field.name().clone()));

            if predicate(field.data_type()) {
                acc.push((
                    Selector(Expr::Path(path.clone())),
                    field.data_type().clone(),
                ));
            }
        }

        // Second pass: recurse into nested structs
        for field in fields {
            if let DataType::Struct(nested_fields) = field.data_type() {
                let mut path = curr.to_vec();
                path.push(Segment::Field(field.name().clone()));
                rec(acc, &path, nested_fields, predicate);
            }
        }
    }

    let DataType::Struct(fields) = datatype else {
        return None;
    };

    let mut result = Vec::new();
    rec(&mut result, &[], fields, &predicate);
    Vec1::try_from_vec(result).ok()
}
