// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// An N-dimensional array of numbers.
///
/// The number of dimensions and their respective lengths is specified by the `shape` field.
/// The dimensions are ordered from outermost to innermost. For example, in the common case of
/// a 2D RGB Image, the shape would be `[height, width, channel]`.
///
/// These dimensions are combined with an index to look up values from the `buffer` field,
/// which stores a contiguous array of typed values.
#[rerun::rerun_type]
#[rust(derive(Default, PartialEq))]
#[rust(repr = "transparent")]
#[rerun(state = "stable")]
pub struct TensorData {
    pub data: rerun::datatypes::TensorData,
}
