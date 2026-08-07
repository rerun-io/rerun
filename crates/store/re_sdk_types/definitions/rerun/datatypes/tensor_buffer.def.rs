// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// The underlying storage for [archetypes.Tensor].
///
/// Tensor elements are stored in a contiguous buffer of a single type.
#[rerun::rerun_type]
#[repr(i8)]
#[rerun(state = "stable")]
#[rust(derive_only(Clone, PartialEq))]
pub enum TensorBuffer {
    /// 8bit unsigned integer.
    U8(Vec<u8>) = 1,

    /// 16bit unsigned integer.
    U16(Vec<u16>) = 2,

    /// 32bit unsigned integer.
    U32(Vec<u32>) = 3,

    /// 64bit unsigned integer.
    U64(Vec<u64>) = 4,

    /// 8bit signed integer.
    I8(Vec<i8>) = 5,

    /// 16bit signed integer.
    I16(Vec<i16>) = 6,

    /// 32bit signed integer.
    I32(Vec<i32>) = 7,

    /// 64bit signed integer.
    I64(Vec<i64>) = 8,

    /// 16bit IEEE-754 floating point, also known as `half`.
    F16(Vec<rerun::f16>) = 9,

    /// 32bit IEEE-754 floating point, also known as `float` or `single`.
    F32(Vec<f32>) = 10,

    /// 64bit IEEE-754 floating point, also known as `double`.
    F64(Vec<f64>) = 11,
}
