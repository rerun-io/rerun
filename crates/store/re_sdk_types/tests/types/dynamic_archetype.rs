use std::collections::BTreeSet;

use re_log_types::datatypes::Utf8;
use re_log_types::{DynamicArchetype, components};

#[test]
fn with_archetype() {
    let values = DynamicArchetype::new("MyExample")
        .with_component::<components::Scalar>("confidence", [1.2f64, 3.4, 5.6])
        .with_component_override::<Utf8>("homepage", "user.url", vec!["https://www.rerun.io"])
        .with_component_from_data(
            "description",
            std::sync::Arc::new(arrow::array::StringArray::from(vec!["Bla bla bla…"])),
        );

    let actual = values
        .as_serialized_batches()
        .into_iter()
        .map(|batch| batch.descriptor)
        .collect::<BTreeSet<_>>();

    assert_eq!(
        actual,
        [
            ComponentDescriptor::partial("confidence")
                .with_builtin_archetype("MyExample")
                .with_component_type(components::Scalar::name()),
            ComponentDescriptor::partial("homepage")
                .with_component_type("user.url".into())
                .with_builtin_archetype("MyExample"),
            ComponentDescriptor::partial("description").with_builtin_archetype("MyExample"),
        ]
        .into_iter()
        .collect()
    );
}
