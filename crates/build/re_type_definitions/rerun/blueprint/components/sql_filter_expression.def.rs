// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// A SQL expression that selects the rows of a table.
///
/// The expression evaluates to a boolean.
/// It is written in the generic SQL dialect of DataFusion, with lambdas such as `x -> x > 1` for list columns.
///
/// Column names are double-quoted, e.g. `"task" ILIKE '%pick%'`.
#[rerun::rerun_type]
#[python(aliases = "str")]
#[python(array_aliases = "str | Sequence[str]")]
#[rerun(scope = "blueprint")]
#[rust(derive(PartialEq, Eq, Hash))]
#[rust(repr = "transparent")]
#[rerun(state = "unstable")]
pub struct SqlFilterExpression {
    pub value: rerun::encodings::Utf8,
}
