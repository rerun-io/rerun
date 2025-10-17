use re_log_types::EntityPath;
use re_test_context::TestContext;
use re_types::ViewClassIdentifier;
use re_viewer_context::{
    BlueprintId, ComponentFallbackProviderResult, DataQueryResult, QueryContext, ViewContext,
};

#[test]
fn test_all_component_fallbacks() {
    let test_context = TestContext::new();

    //
    // EXCLUDE COMPONENTS FROM THE PLACEHOLDER LIST HERE!
    //

    // let excluded_components = [].into_iter().collect::<IntSet<_>>();

    let mut errors = Vec::new();

    test_context.run(&egui::Context::default(), |viewer_context| {
        let view_context = ViewContext {
            viewer_ctx: viewer_context,
            view_id: BlueprintId::invalid(),
            view_class_identifier: ViewClassIdentifier::invalid(),
            view_state: &(),
            query_result: &DataQueryResult::default(),
        };
        for (arch_name, arch) in &test_context.reflection.archetypes {
            let ctx = QueryContext {
                view_ctx: &view_context,
                target_entity_path: &EntityPath::root(),
                archetype_name: Some(*arch_name),
                query: &test_context.blueprint_query,
            };

            for field in &arch.fields {
                let descr = field.component_descriptor(*arch_name);

                let res =
                    test_context
                        .component_fallback_registry
                        .try_fallback_for(&(), &descr, &ctx);

                match res {
                    ComponentFallbackProviderResult::Value(_) => {
                        // Success!
                    }
                    ComponentFallbackProviderResult::ComponentNotHandled => {
                        errors.push((descr, None));
                    }
                    ComponentFallbackProviderResult::SerializationError(err) => {
                        errors.push((descr, Some(err)));
                    }
                }
            }
        }
    });

    if !errors.is_empty() {
        for (descr, err) in errors {
            let t = if let Some(ty) = descr.component_type {
                format!("{descr} of type {ty}")
            } else {
                descr.to_string()
            };
            match err {
                Some(err) => eprintln!("Component {t} had serialization errors: {err}"),
                None => eprintln!("Component {t} not handled."),
            }
        }

        panic!("All components didn't have fallbacks, see stderr.");
    }
}
