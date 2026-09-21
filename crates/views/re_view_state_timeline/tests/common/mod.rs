use re_log_types::{EntityPath, Timeline};
use re_sdk_types::blueprint::encodings::{ComponentSourceKind, VisualizerComponentMapping};
use re_sdk_types::{ComponentIdentifier, Visualizer};
use re_test_context::{TestContext, VisualizerBlueprintContext as _};
use re_test_viewport::TestContextExt as _;
use re_view::execute_systems_for_view;
use re_view_state_timeline::{StateLanesOutput, StateTimelineView, StateVisualizer};
use re_viewer_context::{IdentifiedViewSystem as _, ViewClass as _, ViewId};
use re_viewport_blueprint::{ViewBlueprint, ViewportBlueprint};

const STATE_TARGET: &str = "StateChange:state";

/// Map a custom source component onto the `StateChange:state` slot of a `StateVisualizer`.
///
/// `save_visualizers` bypasses the default auto-spawn heuristics, which only fire when the
/// entity is indicated for the `StateChange` archetype. Custom archetypes (`DynamicArchetype`,
/// `TextLog`) are not indicated, so the visualizer instruction has to be installed explicitly.
pub fn map_source_to_state(source_component: impl Into<ComponentIdentifier>) -> Visualizer {
    map_source_to_state_with_selector(source_component, None)
}

/// Like [`map_source_to_state`] but with a jq-like selector into the source component,
/// e.g. `.u8` to pick a field nested in a struct.
pub fn map_source_to_state_with_selector(
    source_component: impl Into<ComponentIdentifier>,
    selector: Option<&str>,
) -> Visualizer {
    let source_component = source_component.into();
    Visualizer::new(StateVisualizer::identifier().as_str()).with_mappings([
        VisualizerComponentMapping {
            target: STATE_TARGET.into(),
            source_kind: ComponentSourceKind::SourceComponent,
            source_component: Some(source_component.as_str().into()),
            selector: selector.map(Into::into),
        }
        .into(),
    ])
}

/// Lock in the [`Timeline::log_tick`] timeline as active and set up a viewport blueprint
/// with a single view that maps the given visualizers onto `entity`.
///
/// Must be called *after* data has been logged: `set_active_timeline` reads the entity DB
/// when it runs, so the timeline only resolves to a concrete [`Timeline`] (rather than
/// staying [`re_viewer_context::ActiveTimeline::Pending`]) once some data exists on it.
/// After this, [`TestContext::active_timeline`] returns `Some(Timeline::log_tick())`.
pub fn build_view(
    test_context: &mut TestContext,
    entity: &str,
    visualizers: impl IntoIterator<Item = Visualizer>,
) -> ViewId {
    test_context.set_active_timeline(*Timeline::log_tick().name());

    let visualizers: Vec<_> = visualizers.into_iter().collect();
    test_context.setup_viewport_blueprint(|ctx, blueprint| {
        let view = ViewBlueprint::new_with_root_wildcard(StateTimelineView::identifier());
        ctx.save_visualizers(&EntityPath::from(entity), view.id, visualizers);
        blueprint.add_view_at_root(view)
    })
}

pub fn run_visualizer_data(
    test_context: &TestContext,
    view_id: ViewId,
    window: Option<(f64, f64)>,
) -> Vec<StateLanesOutput> {
    run_visualizer_systems(test_context, view_id, window)
        .iter_visualizer_data::<StateLanesOutput>()
        .cloned()
        .collect()
}

/// An explicit window is independent of the time cursor and spans `[min, min + time_spanned]`.
pub fn run_visualizer_systems(
    test_context: &TestContext,
    view_id: ViewId,
    window: Option<(f64, f64)>,
) -> re_viewer_context::SystemExecutionOutput {
    if let Some((min, time_spanned)) = window {
        test_context.with_blueprint_ctx(|ctx, _store_hub| {
            use re_sdk_types::blueprint::{archetypes::TimeAxis, components::LinkAxis};
            use re_sdk_types::encodings::TimeRangeBoundary;
            let property = re_viewport_blueprint::ViewProperty::from_archetype_for_view::<TimeAxis>(
                &ctx, view_id,
            );
            property.save_blueprint_component(
                &ctx,
                &TimeAxis::descriptor_link(),
                &LinkAxis::Independent,
            );
            property.save_blueprint_component(
                &ctx,
                &TimeAxis::descriptor_view_range(),
                &re_sdk_types::blueprint::components::TimeRange(
                    re_sdk_types::encodings::TimeRange {
                        start: TimeRangeBoundary::Absolute(re_view::time_axis_time_from_plot(
                            min.into(),
                            0,
                        )),
                        end: TimeRangeBoundary::Absolute(re_view::time_axis_time_from_plot(
                            (min + time_spanned).into(),
                            0,
                        )),
                    },
                ),
            );
        });
        test_context.handle_system_commands(&egui::Context::default());
    }
    test_context.run_once_in_egui_central_panel(|ctx, _ui| {
        let viewport_blueprint =
            ViewportBlueprint::from_db(ctx.store_context.blueprint, &test_context.blueprint_query);
        let view_blueprint = viewport_blueprint
            .view(&view_id)
            .expect("view should exist in blueprint");
        let class_registry = ctx.view_class_registry();
        let view_class = class_registry.get_class_or_log_error(view_blueprint.class_identifier());
        let view_state = view_class.new_state();
        let once_per_frame = class_registry.run_once_per_frame_context_systems(
            ctx,
            std::iter::once(view_blueprint.class_identifier()),
        );
        let (_view_query, system_output) =
            execute_systems_for_view(ctx, view_blueprint, view_state.as_ref(), &once_per_frame);
        system_output
    })
}

/// Keeps each phase's start time, so tests can assert *when* a reset begins.
/// Gaps are rendered as empty labels.
pub fn timed_phase_labels(lanes_data: &StateLanesOutput, entity: &str) -> Vec<(i64, String)> {
    let group = lanes_data
        .groups
        .iter()
        .find(|g| g.entity_path == EntityPath::from(entity))
        .unwrap_or_else(|| panic!("no lane group for entity {entity}"));
    assert_eq!(
        group.lanes.len(),
        1,
        "expected a single-instance lane group for entity {entity}"
    );
    group.lanes[0]
        .phases
        .iter()
        .map(|p| {
            (
                p.start_time,
                p.content
                    .as_ref()
                    .map_or_else(String::new, |s| s.label.clone()),
            )
        })
        .collect()
}
