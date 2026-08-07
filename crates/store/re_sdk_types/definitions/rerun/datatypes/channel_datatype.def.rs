// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// The innermost datatype of an image.
///
/// How individual color channel components are encoded.
#[rerun::rerun_type]
#[repr(u8)]
#[rerun(state = "stable")]
pub enum ChannelDatatype {
    /// 8-bit unsigned integer.
    #[default]
    U8 = 6,

    /// 8-bit signed integer.
    I8 = 7,

    /// 16-bit unsigned integer.
    U16 = 8,

    /// 16-bit signed integer.
    I16 = 9,

    /// 32-bit unsigned integer.
    U32 = 10,

    /// 32-bit signed integer.
    I32 = 11,

    /// 64-bit unsigned integer.
    U64 = 12,

    /// 64-bit signed integer.
    I64 = 13,

    /// 16-bit IEEE-754 floating point, also known as `half`.
    F16 = 33,

    /// 32-bit IEEE-754 floating point, also known as `float` or `single`.
    F32 = 34,

    /// 64-bit IEEE-754 floating point, also known as `double`.
    F64 = 35,
}
