## `ComponentBatchExt`

```rust
pub trait ComponentBatchExt<'a>
where
    Self: 'a,
{
    // TODO
    fn described_as<A: Archetype>(self) -> ComponentBatchCowWithDescriptor<'a>;
}

// TODO: but how tf does this work when it's ambiguous then (i.e. multiple fields?)
impl<'a> ComponentBatchExt<'a> for &'a dyn ComponentBatch {
    fn described_as<A: Archetype>(self) -> ComponentBatchCowWithDescriptor<'a> {
        let reflection = crate::reflection::get(); // lazycell inside

        let mut descriptor = ComponentDescriptor::new(self.name());
        descriptor.archetype_name = Some(A::name());

        if let Some(arch) = reflection.archetypes.get(&A::name()) {
            descriptor.archetype_field_name = arch
                .fields
                .iter()
                .find_map(|field| (field.component_name == self.name()).then(|| field.name.into()));
        }

        ComponentBatchCowWithDescriptor::new(ComponentBatchCow::Ref(self as &dyn ComponentBatch))
            .with_descriptor_override(descriptor)
    }
}

#[test]
fn huehuehueheuhe() {
    use std::sync::Arc;

    let state = crate::blueprint::components::PanelState::Collapsed;
    dbg!(state.descriptor());

    let state: Arc<dyn ComponentBatch> = Arc::new(state);
    dbg!(state.descriptor());

    let state = state.clone();
    let state = state.clone();
    let state = state.clone();

    let state = state.described_as::<crate::blueprint::archetypes::PanelBlueprint>();
    dbg!(state.descriptor());

    assert!(false); // just to get the output
}

// [crates/store/re_types/src/lib.rs:342:5] state.descriptor() = ComponentDescriptor {
//     archetype_name: None,
//     archetype_field_name: None,
//     component_name: "rerun.blueprint.components.PanelState",
// }
// [crates/store/re_types/src/lib.rs:345:5] state.descriptor() = ComponentDescriptor {
//     archetype_name: None,
//     archetype_field_name: None,
//     component_name: "rerun.blueprint.components.PanelState",
// }
// [crates/store/re_types/src/lib.rs:352:5] state.descriptor() = ComponentDescriptor {
//     archetype_name: Some(
//         "rerun.blueprint.archetypes.PanelBlueprint",
//     ),
//     archetype_field_name: Some(
//         "state",
//     ),
//     component_name: "rerun.blueprint.components.PanelState",
// }
```

This seems unavoidable -- if only because you need it if you're working with fully erased data.
How do you deal with ambiguity I have no idea though.

## `Archetype::update()`

TODO



## Unless?

Should we just keep everything blueprint untagged until we figure out #3381 ?


# Problems

All the blueprint runtime stuff basically implements a very ad-hoc version of 
