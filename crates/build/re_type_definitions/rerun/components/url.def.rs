// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// A URL to a catalog entry, dataset, or segment, as a Rerun URI.
///
/// The Viewer shows it as a button that opens the URI.
/// A relative path is resolved against the server of the table it is shown in.
#[rerun::rerun_type]
#[python(aliases = "str")]
#[python(array_aliases = "str | Sequence[str]")]
#[rerun(state = "unstable")]
#[rust(derive(Default, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[rust(repr = "transparent")]
pub struct Url {
    pub value: rerun::encodings::Utf8,
}
