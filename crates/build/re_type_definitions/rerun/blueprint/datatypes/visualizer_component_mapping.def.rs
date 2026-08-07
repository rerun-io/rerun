// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// What kind of source to use for a visualizer component mapping.
#[rerun::rerun_type]
#[repr(u8)]
#[rerun(scope = "blueprint")]
#[rerun(state = "stable")]
pub enum ComponentSourceKind {
    /// Use an explicit selection defined by `source_component`.
    ///
    /// May or may not make use of a selector string.
    ///
    /// If the source component is not found on the entity,
    /// a heuristically determined value will be used instead.
    // TODO(andreas): this should probably be an error instead (unlike in override/default)?
    SourceComponent = 1,

    /// Use a timeless override value that is defined in the blueprint.
    ///
    /// The override value is stored on the same entity as the visualizer instruction
    /// and uses the `target` as its component name.
    ///
    /// If there is no override value with the target component name,
    /// a heuristically determined value will be used instead.
    Override = 2,

    /// Default as specified on the view's blueprint.
    ///
    /// If the view doesn't specify a default for the target component name,
    /// a heuristically determined value will be used instead.
    Default = 3,
}

/// Associate components of an entity to components of a visualizer.
/// \py
/// \py ⚠ TODO(#12600): The API for component mappings is still evolving, so this may change in the future.
#[rerun::rerun_type]
#[rerun(scope = "blueprint")]
#[rust(derive(PartialEq, Eq))]
#[rerun(state = "unstable")]
pub struct VisualizerComponentMapping {
    /// Target component name which is being mapped to.
    ///
    /// This represents a "slot" on the visualizer.
    pub target: rerun::datatypes::Utf8,

    /// What kind of source to pick.
    pub source_kind: rerun::blueprint::datatypes::ComponentSourceKind,

    /// Component selector for mapping.
    ///
    /// Defaults to `target` if not specified.
    // Uses `String` instead of `Utf8` to allow nulls.
    pub source_component: Option<String>,

    /// Optional selector string using jq-like syntax to pick a specific field on `source_component`.
    ///
    /// Example: ".x" picks a field called "x" from the `source_component` if present.
    ///
    /// Defaults to empty string if not specified.
    // Uses `String` instead of `Utf8` to allow nulls.
    pub selector: Option<String>,
    // Motivation for separating `source_component` and `selector`:
    // Component names may have dots in them, making parsing hard.
    // Example:
    // * component name: "andreas.position"  (where the selector is empty!)
    // * component query: "andreas.position.x" (where "x" is the selector)
    //
    // Counter argument:
    // May cause some UI complexity because we have now to make the distinction there on the fly.
}
