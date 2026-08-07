// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Specifies relation a spatial transform describes.
#[rerun::rerun_type]
#[repr(u8)]
#[rerun(state = "stable")]
pub enum TransformRelation {
    /// The transform describes how to transform into the parent entity's space.
    ///
    /// E.g. a translation of (0, 1, 0) with this [components.TransformRelation] logged at `parent/child` means
    /// that from the point of view of `parent`, `parent/child` is translated 1 unit along `parent`'s Y axis.
    /// From perspective of `parent/child`, the `parent` entity is translated -1 unit along `parent/child`'s Y axis.
    #[default]
    ParentFromChild = 1,

    /// The transform describes how to transform into the child entity's space.
    ///
    /// E.g. a translation of (0, 1, 0) with this [components.TransformRelation] logged at `parent/child` means
    /// that from the point of view of `parent`, `parent/child` is translated -1 unit along `parent`'s Y axis.
    /// From perspective of `parent/child`, the `parent` entity is translated 1 unit along `parent/child`'s Y axis.
    ChildFromParent = 2,
}
