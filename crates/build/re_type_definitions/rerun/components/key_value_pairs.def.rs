// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// A map of string keys to string values.
///
/// This component can be used to attach arbitrary metadata or annotations to entities.
/// Each key-value pair is stored as a UTF-8 string mapping.
#[rerun::rerun_type]
#[python(aliases = "dict[str, str]")]
#[python(array_aliases = "dict[str, str] | Sequence[dict[str, str]]")]
#[rerun(state = "unstable")]
#[rust(derive(Default, PartialEq, Eq))]
pub struct KeyValuePairs {
    /// The key-value pairs that make up this string map.
    pub pairs: Vec<rerun::datatypes::Utf8Pair>,
}
