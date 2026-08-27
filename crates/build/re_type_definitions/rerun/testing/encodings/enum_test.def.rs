// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// A test of the enum type.
#[rerun::rerun_type]
#[repr(u8)]
#[rerun(state = "stable")]
#[rust(arrow_opt)]
pub enum EnumTest {
    /// Great film.
    Up = 1,

    /// Feeling blue.
    Down = 2,

    /// Correct.
    #[default]
    Right = 3,

    /// It's what's remaining.
    Left = 4,

    /// It's the only way to go.
    Forward = 5,

    /// Baby's got it.
    Back = 6,
}

/// Test encoding for fixed-size enum arrays.
#[rerun::rerun_type]
#[arrow(transparent)]
#[rust(derive(Copy, PartialEq, Eq))]
#[rust(repr = "transparent")]
#[rust(tuple_struct)]
#[rerun(state = "stable")]
pub struct FixedSizeEnumArray {
    /// Fixed-size enum array.
    pub values: [rerun::testing::encodings::EnumTest; 3],
}

/// Test encoding for fixed-size arrays of wide enums.
#[rerun::rerun_type]
#[arrow(transparent)]
#[rust(derive(Copy, PartialEq, Eq))]
#[rust(repr = "transparent")]
#[rust(tuple_struct)]
#[rerun(state = "stable")]
pub struct FixedSizeWideEnumArray {
    /// Fixed-size wide enum array.
    pub values: [rerun::testing::encodings::WideEnum; 2],
}

#[rerun::rerun_type]
#[rust(arrow_opt)]
#[rust(derive(Default, PartialEq, Eq))]
#[rerun(state = "stable")]
pub struct MultiEnum {
    /// The first value.
    pub value1: rerun::testing::encodings::EnumTest,

    /// The second value.
    pub value2: Option<rerun::testing::encodings::ValuedEnum>,
}

/// A test of an enumerate with specified values.
#[rerun::rerun_type]
#[repr(u8)]
#[rerun(state = "stable")]
#[rust(arrow_opt)]
pub enum ValuedEnum {
    /// One.
    One = 1,

    /// Two.
    Two = 2,

    /// Three.
    Three = 3,

    /// The answer to life, the universe, and everything.
    TheAnswer = 42,
}

/// A test enum with values that require more than one byte.
#[rerun::rerun_type]
#[repr(u32)]
#[rerun(state = "stable")]
#[rust(arrow_opt)]
pub enum WideEnum {
    /// Low value.
    Low = 1,

    /// High value.
    High = 65536,
}
