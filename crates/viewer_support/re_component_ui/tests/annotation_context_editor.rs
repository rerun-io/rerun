//! Editing the classes of an annotation context through its component editor.

#![expect(clippy::unwrap_used)] // This is only a test

use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use re_component_ui::create_component_ui_registry;
use re_log_types::EntityPath;
use re_sdk_types::encodings::{ClassDescriptionMapElem, Rgba32};
use re_sdk_types::{ComponentBatch as _, archetypes, components};
use re_test_context::TestContext;
use re_viewer_context::external::re_chunk_store::LatestAtQuery;
use re_viewer_context::{EditTarget, blueprint_timeline};

fn annotation_context(classes: [(u16, &str, Rgba32); 2]) -> components::AnnotationContext {
    components::AnnotationContext(
        classes
            .into_iter()
            .map(ClassDescriptionMapElem::from)
            .collect(),
    )
}

#[test]
fn unticking_a_class_makes_it_transparent() {
    let edited = edit(|harness| {
        harness.get_by_label("2 classes").click();
        harness.run();
        harness.snapshot("annotation_context_editor");
        harness
            .get_by_role_and_label(Role::CheckBox, "green")
            .click();
        harness.run();
    });
    assert_eq!(
        edited,
        Some(annotation_context([
            (1, "red", Rgba32::from_rgb(255, 0, 0)),
            (2, "green", Rgba32::from_unmultiplied_rgba(0, 255, 0, 0)),
        ]))
    );
}

#[test]
fn picking_a_color_recolors_the_class() {
    let edited = edit(|harness| {
        harness.get_by_label("2 classes").click();
        harness.run();
        harness.get_by_role_and_label(Role::ColorWell, "red").click();
        harness.run();
        harness.snapshot("annotation_context_editor_color_picker");
        let green_channel = harness.get_all_by_role(Role::SpinButton).nth(1).unwrap();
        green_channel.set_value_accesskit(255.0);
        harness.run();
    });
    assert_eq!(
        edited,
        Some(annotation_context([
            (1, "red", Rgba32::from_rgb(255, 255, 0)),
            (2, "green", Rgba32::from_rgb(0, 255, 0)),
        ]))
    );
}

/// Shows the editor for a red and a green class, lets `interact` with it, and returns what the
/// editor saved, if anything.
fn edit(interact: impl FnOnce(&mut Harness<'_>)) -> Option<components::AnnotationContext> {
    let mut test_context = TestContext::new();
    test_context.component_ui_registry = create_component_ui_registry();
    let entity_path = EntityPath::from("annotations");
    let descriptor = archetypes::AnnotationContext::descriptor_context();
    let logged = annotation_context([
        (1, "red", Rgba32::from_rgb(255, 0, 0)),
        (2, "green", Rgba32::from_rgb(0, 255, 0)),
    ])
    .to_arrow()
    .unwrap();

    let mut harness = test_context
        .setup_kittest_for_rendering_ui([300.0, 300.0])
        .build_ui(|ui| {
            test_context.run(&ui.ctx().clone(), |ctx| {
                ctx.component_ui_registry().try_show_edit_ui(
                    &ctx.blueprint_store_view_ctx(),
                    ui,
                    EditTarget {
                        store_id: ctx.store_context.blueprint.store_id().clone(),
                        timepoint: ctx.store_context.blueprint_timepoint_for_writes(),
                        entity_path: entity_path.clone(),
                    },
                    logged.as_ref(),
                    descriptor.clone(),
                    false,
                );
            });
            test_context.handle_system_commands(ui.ctx());
        });
    harness.run();
    interact(&mut harness);
    drop(harness);

    test_context
        .active_blueprint()
        .latest_at(
            &LatestAtQuery::latest(blueprint_timeline()),
            &entity_path,
            [descriptor.component],
        )
        .component_mono_quiet::<components::AnnotationContext>(descriptor.component)
}
