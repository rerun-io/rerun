use re_log_types::example_components::MyPoint;
use re_test_viewport::{TestSystem, TestView};
use re_viewer_context::{ViewSpawnContext, suggest_view_for_each_entity};

/// Property entities, including their descendants, must not trigger automatic recommendations,
/// but remain eligible when the user explicitly requests a view.
#[test]
fn suggest_view_for_each_entity_respects_spawn_context() {
    use re_log_types::{EntityPath, TimePoint};
    use re_test_context::TestContext;

    let mut test_context = TestContext::new_with_view_class::<TestView>();
    let entities = [
        EntityPath::properties(),
        EntityPath::properties().join(&"child".into()),
        EntityPath::from("points"),
    ];
    for entity in &entities {
        test_context.log_entity(entity.clone(), |builder| {
            builder.with_archetype_auto_row(
                TimePoint::STATIC,
                &re_log_types::example_components::MyPoints::new([MyPoint::new(1.0, 2.0)]),
            )
        });
    }

    test_context.run(&egui::Context::default(), |ctx| {
        let automatic =
            suggest_view_for_each_entity::<TestSystem>(ctx, ViewSpawnContext::Automatic).into_vec();
        assert_eq!(automatic.len(), 1);
        assert_eq!(automatic[0].origin, EntityPath::from("points"));

        let explicit_creation =
            suggest_view_for_each_entity::<TestSystem>(ctx, ViewSpawnContext::Explicit).into_vec();
        assert_eq!(explicit_creation.len(), entities.len());
        for entity in entities {
            assert!(explicit_creation.iter().any(|view| view.origin == entity));
        }
    });
}
