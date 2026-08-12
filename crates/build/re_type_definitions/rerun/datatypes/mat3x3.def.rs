// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// A 3x3 Matrix.
///
/// Matrices in Rerun are stored as flat list of coefficients in column-major order:
/// ```text
///             column 0       column 1       column 2
///        -------------------------------------------------
/// row 0 | flat_columns[0] flat_columns[3] flat_columns[6]
/// row 1 | flat_columns[1] flat_columns[4] flat_columns[7]
/// row 2 | flat_columns[2] flat_columns[5] flat_columns[8]
/// ```
///
/// \py However, construction is done from a list of rows, which follows NumPy's convention:
/// \py ```python
/// \py np.testing.assert_array_equal(
/// \py     rr.datatypes.Mat3x3([1, 2, 3, 4, 5, 6, 7, 8, 9]).flat_columns, np.array([1, 4, 7, 2, 5, 8, 3, 6, 9], dtype=np.float32)
/// \py )
/// \py np.testing.assert_array_equal(
/// \py     rr.datatypes.Mat3x3([[1, 2, 3], [4, 5, 6], [7, 8, 9]]).flat_columns,
/// \py     np.array([1, 4, 7, 2, 5, 8, 3, 6, 9], dtype=np.float32),
/// \py )
/// \py ```
/// \py If you want to construct a matrix from a list of columns instead, use the named `columns` parameter:
/// \py ```python
/// \py np.testing.assert_array_equal(
/// \py     rr.datatypes.Mat3x3(columns=[1, 2, 3, 4, 5, 6, 7, 8, 9]).flat_columns,
/// \py     np.array([1, 2, 3, 4, 5, 6, 7, 8, 9], dtype=np.float32),
/// \py )
/// \py np.testing.assert_array_equal(
/// \py     rr.datatypes.Mat3x3(columns=[[1, 2, 3], [4, 5, 6], [7, 8, 9]]).flat_columns,
/// \py     np.array([1, 2, 3, 4, 5, 6, 7, 8, 9], dtype=np.float32),
/// \py )
/// \py ```
#[rerun::rerun_type]
#[arrow(transparent)]
#[python(aliases = "npt.ArrayLike")]
#[python(array_aliases = "npt.ArrayLike")]
#[rust(derive(Copy, PartialEq, PartialOrd, bytemuck::Pod, bytemuck::Zeroable))]
#[rust(repr = "transparent")]
#[rust(tuple_struct)]
#[rerun(state = "stable")]
pub struct Mat3x3 {
    /// Flat list of matrix coefficients in column-major order.
    pub flat_columns: [f32; 9],
}
