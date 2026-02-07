use re_sdk_types::ViewClassIdentifier;
use re_sdk_types::external::arrow::util::display::{ArrayFormatter, FormatOptions};
use re_test_context::TestContext;
use re_viewer_context::{BlueprintId, DataQueryResult, QueryContext, ViewContext};

#[test]
fn test_all_component_fallbacks() {
    let test_context = TestContext::new();

    test_context.run(&egui::Context::default(), |viewer_context| {
        // Create a dummy view context
        let space_origin = "/".into();
        let view_context = ViewContext {
            viewer_ctx: viewer_context,
            view_id: BlueprintId::invalid(),
            view_class_identifier: ViewClassIdentifier::invalid(),
            space_origin: &space_origin,
            view_state: &(),
            query_result: &DataQueryResult::default(),
        };

        // We do snapshot tests for all archetype field fallbacks.
        for (arch_name, arch) in &test_context.reflection.archetypes {
            let ctx = QueryContext {
                view_ctx: &view_context,
                target_entity_path: &"/stockholm/södermalm/slussen".into(),
                instruction_id: None,
                archetype_name: Some(*arch_name),
                query: &test_context.blueprint_query,
            };
            let mut arch_display = String::new();

            for field in &arch.fields {
                let descr = field.component_descriptor(*arch_name);

                let res = test_context.component_fallback_registry.fallback_for(
                    descr.component,
                    descr.component_type,
                    &ctx,
                );

                let formatter =
                    ArrayFormatter::try_new(&res, &FormatOptions::default().with_null("null"))
                        .unwrap();

                let values = (0..res.len())
                    .map(|i| formatter.value(i).to_string())
                    .collect::<Vec<_>>()
                    .join(", ");

                arch_display.push_str(&format!("{}: [{values}]\n", field.name));
            }

            let name = format!("arch_fallback_{arch_name}");
            insta::assert_snapshot!(name, arch_display);
        }
    });
}
