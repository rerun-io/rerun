//! Integration test for heuristic mappings in the `TextDocumentView`.
//!
//! Note that some of the behavior tested here is fairly fundamental and not specific to the text view itself.
//! Text view is well suited for this though since we have simple built-in heuristics for it and it's naturally not a very complex view with custom rules etc..

use std::sync::Arc;

use re_chunk_store::LatestAtQuery;
use re_log_types::external::arrow::array::{ArrayRef, StringArray, StructArray};
use re_log_types::{EntityPath, Timeline, TimelineName};
use re_sdk_types::archetypes::{TextDocument, TextLog};
use re_sdk_types::blueprint::encodings::{ComponentSourceKind, VisualizerComponentMapping};
use re_sdk_types::components::Text;
use re_sdk_types::{ComponentDescriptor, RowId, VisualizableArchetype as _};
use re_test_context::{TestContext, VisualizerBlueprintContext as _};
use re_test_viewport::TestContextExt as _;
use re_view_text_document::TextDocumentView;
use re_viewer_context::{ViewClass as _, ViewContext, ViewId, VisualizerComponentSource};
use re_viewport_blueprint::ViewBlueprint;

const ROS_STRING_MESSAGE_COMPONENT: &str = "std_msgs.msg.String:message";

fn raw_text_context() -> TestContext {
    let mut context = TestContext::new_with_view_class::<TextDocumentView>();
    for (tick, value) in [(1, "first"), (2, "latest")] {
        let input = StructArray::try_from(vec![
            ("data", Arc::new(StringArray::from(vec![value])) as ArrayRef),
            (
                "alternate",
                Arc::new(StringArray::from(vec!["explicit"])) as ArrayRef,
            ),
        ])
        .expect("text fields have matching lengths");
        context.log_entity("raw", |builder| {
            builder.with_row(
                RowId::new(),
                [(Timeline::log_tick(), tick)],
                [(
                    ComponentDescriptor::partial(ROS_STRING_MESSAGE_COMPONENT),
                    Arc::new(input) as ArrayRef,
                )],
            )
        });
    }
    context
}

/// Makes sure that we handle native & heuristic matches correctly when occurring on the same entity.
#[test]
fn native_text_sources_take_precedence_over_raw_text() {
    for native in [None, Some(TextLog::descriptor_text())] {
        let mut context = raw_text_context();
        let expected_value = if native.is_some() { "native" } else { "latest" };
        let expected_source = match &native {
            Some(descriptor) => Some(VisualizerComponentSource::simple_map(descriptor.component)),
            None => Some(VisualizerComponentSource::SourceComponent {
                source_component: ROS_STRING_MESSAGE_COMPONENT.into(),
                selector: ".data".into(),
            }),
        };
        if let Some(descriptor) = native {
            // `TextLog:text` is a separate component on the same entity, not a descriptor or
            // schema change to the existing raw message component. Both sources coexist.
            context.log_entity("raw", |builder| {
                builder.with_row(
                    RowId::new(),
                    [(Timeline::log_tick(), 2)],
                    [(
                        descriptor,
                        Arc::new(StringArray::from(vec!["native"])) as ArrayRef,
                    )],
                )
            });
        }
        let view_id = context.setup_viewport_blueprint(|_ctx, blueprint| {
            blueprint.add_view_at_root(ViewBlueprint::new_with_root_wildcard(
                TextDocumentView::identifier(),
            ))
        });
        assert_latest_text(&context, view_id, expected_source, expected_value);
    }
}

// Make sure that a blueprint persisted mapping wins even if the data comes in later on.
#[test]
fn persisted_mapping_survives_heuristic_recomputation() {
    let mut context = raw_text_context();
    let entity = EntityPath::from("raw");
    let target = TextDocument::descriptor_text().component;
    let view_id = context.setup_viewport_blueprint(|_ctx, blueprint| {
        blueprint.add_view_at_root(ViewBlueprint::new_with_root_wildcard(
            TextDocumentView::identifier(),
        ))
    });
    context.setup_viewport_blueprint(|ctx, _blueprint| {
        ctx.save_visualizers(
            &entity,
            view_id,
            [TextDocument::update_fields().visualizer().with_mappings([
                VisualizerComponentMapping {
                    target: target.as_str().into(),
                    source_kind: ComponentSourceKind::SourceComponent,
                    source_component: Some(ROS_STRING_MESSAGE_COMPONENT.into()),
                    selector: Some(".alternate".into()),
                }
                .into(),
            ])],
        );
    });
    context.setup_viewport_blueprint(|_ctx, _blueprint| {});

    assert_latest_text(
        &context,
        view_id,
        Some(VisualizerComponentSource::SourceComponent {
            source_component: ROS_STRING_MESSAGE_COMPONENT.into(),
            selector: ".alternate".into(),
        }),
        "explicit",
    );
}

fn assert_latest_text(
    context: &TestContext,
    view_id: ViewId,
    source: Option<VisualizerComponentSource>,
    expected: &str,
) {
    let target = TextDocument::descriptor_text().component;
    let query_result = &context.query_results[&view_id];
    let result = query_result
        .tree
        .lookup_result_by_path(EntityPath::from("raw").hash())
        .expect("raw text should be visualizable");
    assert_eq!(result.visualizer_instructions.len(), 1);
    let instruction = &result.visualizer_instructions[0];
    assert_eq!(
        instruction.component_mappings,
        source.into_iter().map(|source| (target, source)).collect()
    );
    context.run(&egui::Context::default(), |viewer_ctx| {
        let ctx = ViewContext {
            viewer_ctx,
            view_id,
            view_class_identifier: TextDocumentView::identifier(),
            space_origin: &EntityPath::root(),
            view_state: &(),
            query_result,
        };
        let latest = re_view::latest_at_with_blueprint_resolved_data(
            &ctx,
            None,
            &LatestAtQuery::new(TimelineName::log_tick(), 2),
            result,
            [target],
            Some(instruction),
        );
        assert_eq!(latest.get_mono::<Text>(target), Some(Text::from(expected)));
    });
}
