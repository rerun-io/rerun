// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Unit of a measured value, e.g. `"Pa"`, `"lux"`, `"°C"`, `"m"`.
///
/// Used for display only. It does not convert or scale the value.
#[rerun::rerun_type]
#[arrow(transparent)]
#[docs(unreleased)]
#[python(aliases = "str")]
#[python(array_aliases = "str | Sequence[str]")]
#[rerun(state = "unstable")]
#[rust(derive(PartialEq, Eq, PartialOrd, Ord))]
#[rust(repr = "transparent")]
pub struct Unit {
    pub value: rerun::encodings::Utf8,
}
