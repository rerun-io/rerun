// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

// TODO(#2388): need a bunch of mime constants in here

/// A standardized media type (RFC2046, formerly known as MIME types), encoded as a string.
///
/// The complete reference of officially registered media types is maintained by the IANA and can be
/// consulted at <https://www.iana.org/assignments/media-types/media-types.xhtml>.
#[rerun::rerun_type]
#[python(aliases = "str")]
#[python(array_aliases = "str | Sequence[str]")]
#[rust(derive(PartialEq, Eq, PartialOrd, Ord, Hash))]
#[rust(repr = "transparent")]
#[rerun(state = "stable")]
pub struct MediaType {
    pub value: rerun::datatypes::Utf8,
}
