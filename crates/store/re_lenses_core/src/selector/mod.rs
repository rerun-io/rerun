//! Selector API for parsing and executing [`jq`](https://github.com/jqlang/jq/)-like queries on Arrow arrays.
//!
//! This module provides a high-level path-based API, but in contrast to `jq` its semantics are **columnar**,
//! following Apache Arrow's data model rather than a row-oriented object model.
//!
//! # Syntax
//!
//! The selector syntax is a subset of `jq`:
//!
//! | Syntax    | Meaning                                                 | Example            |
//! |-----------|---------------------------------------------------------|--------------------|
//! | `.field`  | Access a named field in a struct                        | `.location`        |
//! | `[]`      | Iterate over every element of a list                    | `.poses[]`         |
//! | `[N]`     | Index into a list by position                           | `.[0]`             |
//! | `?`       | Error suppression / optional operator                   | `.field?`          |
//! | `!`       | Assert non-null (promotes all-null rows to outer nulls) | `.field!`          |
//! | `\|`      | Pipe the output of one expression to another            | `.foo \| .bar`     |
//! | `pack(…)` | Pack 1:1 paths into a `FixedSizeList` (see below)        | `pack(.x, .y, .z)` |
//!
//! Segments can be chained without an explicit pipe: `.poses[].x` is equivalent to `.poses[] | .x`.
//!
//! # Differences from `jq`
//!
//! * **Columnar, not row-oriented** - operations apply to entire Arrow columns rather than individual JSON values.
//! * **No filters or arithmetic** - only path navigation, iteration, and built-in functions are supported.
//! * **No quoted field names or string interpolation** - field names must be bare identifiers
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
//!
//! # Packing into fixed-size lists
//!
//! `pack(path, path, …)` packs several path expressions into a [`FixedSizeList`](arrow::array::FixedSizeListArray)
//! of size equal to the number of paths. This is the canonical way to build component-style
//! columns such as `Position3D` (`FixedSizeList<f32>[3]`) from separate scalar fields:
//! `pack(.x, .y, .z)`.
//!
//! Each path is evaluated against `pack`'s input and must produce **exactly one value per row**
//! with the **same datatype** (nullability may differ). The paths are zipped per row, so the
//! result has the same number of rows as the input.
//!
//! Nullability follows an **entry-level AND** model: an entry is null if **any** of its paths is
//! null (a missing component nulls the whole entry). Because a null in one path therefore shadows
//! the valid values of its siblings, this must be acknowledged:
//!
//! * A **non-nullable** path needs no annotation.
//! * A **nullable** path must be marked with `!` (e.g. `pack(.x, .y!, .z)`), otherwise `pack` errors.
//!   This requirement is **type-driven** — it depends on whether the path is nullable in the schema,
//!   not on whether a particular batch happens to contain nulls — so a given `pack` either always
//!   validates or always errors, regardless of data.
//!
//! The resulting `FixedSizeList` (and its element field) is nullable iff any path is nullable, and
//! an element-level null only ever occurs under a null entry.
//!
//! # Anonymous functions
//!
//! Using [`Selector::pipe`] it is possible to chain anonymous functions to selectors. The result will be a
//! [`Selector<DynExpr>`], which can be executed just like a regular selector.

mod dyn_expr;
mod eval;
mod lexer;
mod parser;
mod runtime;

pub mod function_registry;

pub use dyn_expr::DynExpr;
pub use parser::Literal;
pub use runtime::Runtime;

use arrow::{
    array::{ArrayRef, ListArray},
    datatypes::{DataType, Fields},
};
use vec1::Vec1;

use parser::Expr;

/// A parsed selector expression that can be executed against Arrow arrays.
#[derive(Clone, re_byte_size::SizeBytes)]
pub struct Selector<E = Expr> {
    expr: E,
}

impl std::fmt::Debug for Selector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self { expr } = self;

        f.debug_struct("Selector").field("expr", expr).finish()
    }
}

impl PartialEq for Selector {
    fn eq(&self, other: &Self) -> bool {
        let Self { expr } = self;

        *expr == other.expr
    }
}

impl Eq for Selector {}

impl std::hash::Hash for Selector {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        let Self { expr } = self;

        expr.hash(state);
    }
}

impl std::fmt::Display for Selector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.expr)
    }
}

impl Selector {
    /// Create a new selector.
    pub fn new(expr: Expr) -> Self {
        Self { expr }
    }

    /// Parse a selector from a query string.
    ///
    /// This is a convenience wrapper around [`FromStr`](std::str::FromStr).
    pub fn parse(query: &str) -> Result<Self, Error> {
        query.parse()
    }
}

/// Implementors can be converted into [`DynExpr`] and therefore be piped into [`Selector`]s.
pub trait IntoDynExpr {
    fn into_dyn_expr(self) -> DynExpr;
}

impl<
    F: Fn(&ArrayRef) -> Result<Option<ArrayRef>, crate::combinators::Error> + Send + Sync + 'static,
> IntoDynExpr for F
{
    fn into_dyn_expr(self) -> DynExpr {
        DynExpr::Function(std::sync::Arc::new(self))
    }
}

impl IntoDynExpr for Selector {
    fn into_dyn_expr(self) -> DynExpr {
        let Self { expr } = self;
        DynExpr::Expr(expr)
    }
}

impl IntoDynExpr for Selector<DynExpr> {
    fn into_dyn_expr(self) -> DynExpr {
        let Self { expr } = self;
        expr
    }
}

impl<E: eval::Eval + Into<DynExpr>> Selector<E> {
    /// Execute this selector against a raw array using the default runtime.
    ///
    /// This is the `ArrayRef`-based entry point. For per-row execution
    /// on a [`ListArray`], use [`execute_per_row`](Self::execute_per_row).
    ///
    /// To execute with a custom runtime, use [`Runtime::execute`] directly.
    pub fn execute(&self, source: ArrayRef) -> Result<Option<ArrayRef>, Error> {
        runtime::default_runtime().execute(self, source)
    }

    /// Execute this selector against each row of a [`ListArray`] using the default runtime.
    ///
    /// Performs implicit iteration over the inner list array, and reconstructs the array at the end.
    ///
    /// `map(.poses[].x)` is the actual query, we only require writing the `.poses[].x` portion.
    ///
    /// The output is guaranteed to have the same number of rows as the input.
    ///
    /// Returns `None` if the expression's error was suppressed (e.g. `.field?`).
    ///
    /// To execute with a custom runtime, use [`Runtime::execute_per_row`] directly.
    pub fn execute_per_row(&self, source: &ListArray) -> Result<Option<ListArray>, Error> {
        runtime::default_runtime().execute_per_row(self, source)
    }

    /// Pipe this selector into another expression, producing a [`Selector<DynExpr>`].
    ///
    /// Accepts any type that converts into a [`DynExpr`], including [`Selector<Expr>`] and
    /// [`Selector<DynExpr>`], and anonymous functions that operate on [`ArrayRef`].
    pub fn pipe(self, rhs: impl IntoDynExpr) -> Selector<DynExpr> {
        Selector {
            expr: DynExpr::Pipe {
                left: Box::new(self.expr.into()),
                right: Box::new(rhs.into_dyn_expr()),
            },
        }
    }
}

impl From<Selector> for Selector<DynExpr> {
    fn from(selector: Selector) -> Self {
        Self {
            expr: DynExpr::from(selector.expr),
        }
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

/// Fold an iterator of `Expr` into a left-associative chain of implicit pipes.
fn chain(exprs: impl IntoIterator<Item = Expr>) -> Expr {
    let mut iter = exprs.into_iter();
    let Some(first) = iter.next() else {
        return Expr::Identity;
    };
    iter.fold(first, |left, right| Expr::Pipe {
        left: Box::new(left),
        right: Box::new(right),
        implicit: true,
    })
}

/// Dispatch a single datatype: enqueue structs, unwrap lists, or check the predicate.
fn process_datatype<'a, P>(
    mut path: Vec<Expr>,
    datatype: &'a DataType,
    predicate: &P,
    result: &mut Vec<(Selector, DataType)>,
    queue: &mut std::collections::VecDeque<(Vec<Expr>, &'a Fields)>,
) where
    P: Fn(&DataType) -> bool,
{
    match datatype {
        dt if predicate(dt) => {
            result.push((Selector::new(chain(path)), dt.clone()));
        }
        DataType::Struct(fields) => {
            queue.push_back((path, fields));
        }
        DataType::List(inner) | DataType::FixedSizeList(inner, ..) => {
            path.push(Expr::Each);
            match inner.data_type() {
                dt if predicate(dt) => {
                    result.push((Selector::new(chain(path)), dt.clone()));
                }
                DataType::Struct(nested_fields) => {
                    queue.push_back((path, nested_fields));
                }
                DataType::FixedSizeList(field, ..) => {
                    let dt = field.data_type();
                    if predicate(dt) {
                        path.push(Expr::Each);
                        result.push((Selector::new(chain(path)), dt.clone()));
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
    let mut queue: std::collections::VecDeque<(Vec<Expr>, &Fields)> =
        std::collections::VecDeque::new();

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
            field_path.push(Expr::Field(field.name().clone()));
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
