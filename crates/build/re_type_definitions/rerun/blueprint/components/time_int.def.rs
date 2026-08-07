// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// A reference to a time.
#[rerun::rerun_type]
#[arrow(transparent)]
#[python(aliases = "long")]
#[rerun(scope = "blueprint")]
#[rust(derive(Copy, PartialEq, Eq, PartialOrd, Ord))]
#[rust(repr = "transparent")]
#[rust(tuple_struct)]
#[rerun(state = "unstable")]
pub struct TimeInt {
    pub time: rerun::datatypes::TimeInt,
}
