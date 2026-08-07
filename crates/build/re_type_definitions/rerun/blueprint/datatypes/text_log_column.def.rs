// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// A text log column.
#[rerun::rerun_type]
#[python(aliases = "blueprint_datatypes.TextLogColumnKindLike")]
#[rerun(scope = "blueprint")]
#[rust(derive(PartialEq, Eq, Hash))]
#[rerun(state = "unstable")]
pub struct TextLogColumn {
    /// Is this column visible?
    ///
    /// Defaults to true.
    pub visible: rerun::datatypes::Bool,

    /// What kind of column is this?
    pub kind: rerun::blueprint::datatypes::TextLogColumnKind,
}

/// A text log column kind.
#[rerun::rerun_type]
#[repr(u8)]
#[rerun(scope = "blueprint")]
#[rerun(state = "stable")]
pub enum TextLogColumnKind {
    /// Which entity path this was logged to.
    #[default]
    EntityPath = 1,

    /// The log level, i.e INFO, WARN, ERROR.
    LogLevel = 2,

    /// The text message the log has.
    Body = 3,
}
