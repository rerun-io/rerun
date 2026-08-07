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
///
/// \py It's not currently possible to use `send_columns` with tensors since construction
/// \py of `rerun.components.TensorDataBatch` does not support more than a single element.
/// \py This will be addressed as part of <https://github.com/rerun-io/rerun/issues/6832>.
#[rerun::rerun_type]
#[python(aliases = "npt.ArrayLike")]
#[python(array_aliases = "npt.ArrayLike")]
#[rerun(state = "stable")]
#[rust(derive(PartialEq,))]
pub struct TensorData {
    /// The shape of the tensor, i.e. the length of each dimension.
    pub shape: Vec<u64>,

    /// The names of the dimensions of the tensor (optional).
    ///
    /// If set, should be the same length as [datatypes.TensorData.shape].
    /// If it has a different length your names may show up improperly,
    /// and some constructors may produce a warning or even an error.
    ///
    /// Example: `["height", "width", "channel", "batch"]`.
    pub names: Option<Vec<String>>,

    /// The content/data.
    pub buffer: rerun::datatypes::TensorBuffer,
}
