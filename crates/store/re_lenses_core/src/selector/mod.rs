//! Selector API for parsing and executing [`jq`](https://github.com/jqlang/jq/)-like queries on Arrow arrays.
//!
//! This module provides a high-level path-based API, but in contrast to `jq` its semantics are **columnar**,
//! following Apache Arrow's data model rather than a row-oriented object model.
//!
//! # Syntax
//!
//! The selector syntax is a subset of `jq`:
//!
//! | Syntax      | Meaning                                                    | Example        |
//! |-------------|------------------------------------------------------------|----------------|
//! | `.field`    | Access a named field in a struct                           | `.location`    |
//! | `[]`        | Iterate over every element of a list                       | `.poses[]`     |
//! | `[N]`       | Index into a list by position                              | `.[0]`         |
//! | `?`         | Error suppression / optional operator                      | `.field?`      |
//! | `!`         | Assert non-null (promotes all-null rows to outer nulls)    | `.field!`      |
//! | `\|`        | Pipe the output of one expression to another               | `.foo \| .bar` |
//!
//! Segments can be chained without an explicit pipe: `.poses[].x` is equivalent to `.poses[] | .x`.
//!
//! # Differences from `jq`
//!
//! * **Columnar, not row-oriented** — operations apply to entire Arrow columns rather than individual JSON values.
//! * **No filters, arithmetic, or built-in functions** — only path navigation and iteration are supported.
//! * **No quoted field names or string interpolation** — field names must be bare identifiers
//!   (alphanumeric, `-`, `_`).
//!
//! # Protobuf and null handling
//!
//! The `?` and `!` operators exist primarily to handle Arrow columns produced from protobuf
//! messages. Proto3 `optional` fields have **presence tracking**: when a field is unset the
//! corresponding Arrow column contains `null` rather than the type's default value. Navigating
//! into a struct with optional sub-fields can therefore yield lists whose inner values are all
//! null (e.g. `[null]` instead of a top-level `null`).
//!
//! * `?` suppresses errors when a field is entirely absent from the schema, which happens
//!   during schema evolution or when optional columns are omitted.
//! * `!` promotes rows where **all** inner values are null to an outer null, collapsing
//!   `[null]` → `null` so downstream consumers see clean nullability.

mod lexer;
mod parser;
mod runtime;

pub mod function_registry;

use std::sync::Arc;

pub use parser::Literal;
pub use runtime::Runtime;

use arrow::{
    array::ListArray,
    datatypes::{DataType, Fields},
};
use vec1::Vec1;

use parser::{Expr, Segment, SegmentKind};

/// A parsed selector expression that can be executed against Arrow arrays.
#[derive(Clone)]
pub struct Selector {
    expr: Expr,
    runtime: Arc<Runtime>,
}

impl re_byte_size::SizeBytes for Selector {
    fn heap_size_bytes(&self) -> u64 {
        let Self { expr, runtime } = self;

        expr.heap_size_bytes() + runtime.heap_size_bytes()
    }
}

impl std::fmt::Debug for Selector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self { expr, runtime: _ } = self;

        f.debug_struct("Selector").field("expr", expr).finish()
    }
}

impl PartialEq for Selector {
    fn eq(&self, other: &Self) -> bool {
        let Self { expr, runtime } = self;

        *expr == other.expr && Arc::ptr_eq(runtime, &other.runtime)
    }
}

impl Eq for Selector {}

impl std::hash::Hash for Selector {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        let Self { expr, runtime } = self;

        expr.hash(state);
        Arc::as_ptr(runtime).hash(state);
    }
}

impl std::fmt::Display for Selector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.expr)
    }
}

impl Selector {
    /// Create a new selector using the default runtime.
    pub fn new(expr: Expr) -> Self {
        Self {
            expr,
            runtime: runtime::default_runtime(),
        }
    }

    /// Parse a selector from a query string.
    ///
    /// This is a convenience wrapper around [`FromStr`](std::str::FromStr).
    pub fn parse(query: &str) -> Result<Self, Error> {
        query.parse()
    }

    /// Execute this selector against each row of a [`ListArray`].
    ///
    /// Performs implicit iteration over the inner list array, and reconstructs the array at the end.
    ///
    /// `[.[].poses[].x]` is the actual query, we only require writing the `.poses[].x` portion.
    ///
    /// Returns `None` if the expression's error was suppressed (e.g. `.field?`).
    pub fn execute_per_row(&self, source: &ListArray) -> Result<Option<ListArray>, Error> {
        runtime::execute_per_row(&self.expr, source, &self.runtime).map_err(Into::into)
    }

    /// Set which runtime this selector should run with.
    pub fn with_runtime(mut self, runtime: impl Into<Arc<Runtime>>) -> Self {
        self.runtime = runtime.into();
        self
    }
}

impl crate::combinators::Transform for Selector {
    type Source = ListArray;
    type Target = ListArray;

    fn transform(
        &self,
        source: &Self::Source,
    ) -> Result<Option<Self::Target>, crate::combinators::Error> {
        runtime::execute_per_row(&self.expr, source, &self.runtime)
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

        Ok(Self::new(expr))
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
    Runtime(#[from] crate::combinators::Error),
}

/// Dispatch a single datatype: enqueue structs, unwrap lists, or check the predicate.
fn process_datatype<'a, P>(
    mut path: Vec<Segment>,
    datatype: &'a DataType,
    predicate: &P,
    result: &mut Vec<(Selector, DataType)>,
    queue: &mut std::collections::VecDeque<(Vec<Segment>, &'a Fields)>,
) where
    P: Fn(&DataType) -> bool,
{
    match datatype {
        dt if predicate(dt) => {
            result.push((Selector::new(Expr::Path(path)), dt.clone()));
        }
        DataType::Struct(fields) => {
            queue.push_back((path, fields));
        }
        DataType::List(inner) | DataType::FixedSizeList(inner, ..) => {
            path.push(Segment {
                kind: SegmentKind::Each,
                suppressed: false,
                assert_non_null: false,
            });
            match inner.data_type() {
                dt if predicate(dt) => {
                    result.push((Selector::new(Expr::Path(path)), dt.clone()));
                }
                DataType::Struct(nested_fields) => {
                    queue.push_back((path, nested_fields));
                }
                DataType::FixedSizeList(field, ..) => {
                    let dt = field.data_type();
                    if predicate(dt) {
                        path.push(Segment {
                            kind: SegmentKind::Each,
                            suppressed: false,
                            assert_non_null: false,
                        });
                        result.push((Selector::new(Expr::Path(path)), dt.clone()));
                    }
                }
                _ => {}
            }
        }
        _ => {}
    }
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
    let mut result = Vec::new();
    let mut queue = std::collections::VecDeque::new();

    match datatype {
        DataType::Struct(_) | DataType::List(_) | DataType::FixedSizeList(..) => {
            process_datatype(Vec::new(), datatype, &predicate, &mut result, &mut queue);
        }
        _ => return None,
    }

    // Breadth-first traversal
    while let Some((path, fields)) = queue.pop_front() {
        for field in fields {
            let mut field_path = path.clone();
            field_path.push(Segment {
                kind: SegmentKind::Field(field.name().clone()),
                suppressed: false,
                assert_non_null: false,
            });
            process_datatype(
                field_path,
                field.data_type(),
                &predicate,
                &mut result,
                &mut queue,
            );
        }
    }

    Vec1::try_from_vec(result).ok()
}
