// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// General visualization behavior of an entity.
///
/// TODO(#6541): Fields of this archetype currently only have an effect when logged in the blueprint store.
///
/// \example archetypes/entity_behavior missing="cpp,rs" image="https://static.rerun.io/entity_behavior/831ccdaba769608408edb5edbfaaecf604b53d69/1200w.png"
#[rerun::rerun_type]
#[docs(category = "General")]
#[rerun(scope = "blueprint")]
#[rerun(state = "stable")]
pub struct EntityBehavior {
    /// Whether the entity can be interacted with.
    ///
    /// This property is propagated down the entity hierarchy until another child entity
    /// sets `interactive` to a different value at which point propagation continues with that value instead.
    ///
    /// Defaults to parent's `interactive` value or true if there is no parent.
    #[rerun(optional)]
    pub interactive: Option<rerun::components::Interactive>,

    /// Whether the entity is visible.
    ///
    /// This property is propagated down the entity hierarchy until another child entity
    /// sets `visible` to a different value at which point propagation continues with that value instead.
    ///
    /// Defaults to parent's `visible` value or true if there is no parent.
    #[rerun(optional)]
    pub visible: Option<rerun::components::Visible>,
}
