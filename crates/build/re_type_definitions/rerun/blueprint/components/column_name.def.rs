// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// The name of a column in a table.
///
/// This is the physical column name: it is what the column is looked up by, and it is also
/// what the user reads whenever the column has no separate, human-facing label.
#[rerun::rerun_type]
#[python(aliases = "str")]
#[python(array_aliases = "str | Sequence[str]")]
#[rerun(scope = "blueprint")]
#[rust(derive(
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    ::serde::Serialize,
    ::serde::Deserialize
))]
#[rust(repr = "transparent")]
#[rerun(state = "unstable")]
pub struct ColumnName {
    pub value: rerun::encodings::Utf8,
}
