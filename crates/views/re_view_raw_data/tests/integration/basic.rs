use std::sync::Arc;

use re_chunk_store::RowId;
use re_log_types::example_components::{MyColor, MyPoint, MyPoints};
use re_log_types::{EntityPath, TimePoint, Timeline};
use re_sdk_types::blueprint;
use re_sdk_types::blueprint::archetypes::{RawDataRows, RawDataViewFormat};
use re_sdk_types::components::Visible;
use re_test_context::TestContext;
use re_test_context::external::egui_kittest;
use re_test_context::external::egui_kittest::kittest::Queryable as _;
use re_test_viewport::TestContextExt as _;
use re_view_raw_data::RawDataView;
use re_viewer_context::{RecommendedView, ViewClass as _, ViewClassExt as _, ViewId};
use re_viewport_blueprint::{ViewBlueprint, ViewProperty};

type Harness<'a> = egui_kittest::Harness<'a>;

const ENTITY: &str = "points";

fn frame_timeline() -> Timeline {
    Timeline::new_sequence("frame")
}

/// Points at frames 10 and 30, colors at frame 20.
fn test_context() -> TestContext {
    let mut test_context = TestContext::new_with_view_class::<RawDataView>();

    // The property panel edits blueprint components, which need their editors to be anything but
    // read-only text.
    test_context.component_ui_registry = re_component_ui::create_component_ui_registry();
    re_data_ui::register_component_uis(&mut test_context.component_ui_registry);

    let timeline = frame_timeline();
    for frame in [10, 30] {
        test_context.log_entity(ENTITY, |builder| {
            builder.with_archetype(
                RowId::new(),
                TimePoint::from([(timeline, frame)]),
                &MyPoints::new([MyPoint::new(frame as f32, 1.0)]),
            )
        });
    }
    test_context.log_entity(ENTITY, |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::from([(timeline, 20)]),
            &MyPoints::update_fields().with_colors([MyColor::from_rgb(255, 0, 0)]),
        )
    });

    test_context.set_active_timeline(*timeline.name());
    test_context
}

fn log_text(test_context: &mut TestContext, entity_path: &str) {
    test_context.log_entity(entity_path, |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::from([(frame_timeline(), 10)]),
            &re_sdk_types::archetypes::TextLog::new("message"),
        )
    });
}

/// A raw data view, with its blueprint properties written by `configure`.
fn setup_view(
    test_context: &mut TestContext,
    recommended: RecommendedView,
    configure: impl FnOnce(&re_viewer_context::ViewerContext<'_>, ViewId),
) -> ViewId {
    test_context.setup_viewport_blueprint(|ctx, blueprint| {
        let view = ViewBlueprint::new(RawDataView::identifier(), recommended);
        configure(ctx, view.id);
        blueprint.add_view_at_root(view)
    })
}

/// A raw view over `ENTITY` with no explicit configuration.
fn setup_default_view(test_context: &mut TestContext) -> ViewId {
    setup_view(
        test_context,
        RecommendedView::new_single_entity(EntityPath::from(ENTITY)),
        |_, _| {},
    )
}

/// A raw view over `ENTITY` with one component of its blueprint property `A` set.
fn setup_view_with_property<A: re_sdk_types::Archetype>(
    test_context: &mut TestContext,
    descriptor: &re_sdk_types::ComponentDescriptor,
    values: &dyn re_sdk_types::ComponentBatch,
) -> ViewId {
    setup_view(
        test_context,
        RecommendedView::new_single_entity(EntityPath::from(ENTITY)),
        |ctx, view_id| {
            ViewProperty::from_archetype_for_view::<A>(ctx, view_id)
                .save_blueprint_component(ctx, descriptor, values);
        },
    )
}

/// A view over `ENTITY` configured to show `components`.
fn setup_view_with_rows(
    test_context: &mut TestContext,
    components: &[re_sdk_types::ComponentIdentifier],
) -> ViewId {
    let rows: Vec<_> = components
        .iter()
        .map(|c| blueprint::components::ComponentIdentifier::from(c.as_str()))
        .collect();
    setup_view_with_property::<RawDataRows>(
        test_context,
        &RawDataRows::descriptor_data_rows(),
        &rows,
    )
}

fn points() -> re_sdk_types::ComponentIdentifier {
    MyPoints::descriptor_points().component
}

fn colors() -> re_sdk_types::ComponentIdentifier {
    MyPoints::descriptor_colors().component
}

/// Render the view itself.
fn view_harness(test_context: &TestContext, view_id: ViewId) -> Harness<'_> {
    view_harness_with_size(test_context, view_id, egui::vec2(500.0, 400.0))
}

fn view_harness_with_size(
    test_context: &TestContext,
    view_id: ViewId,
    size: egui::Vec2,
) -> Harness<'_> {
    let mut harness = test_context
        .setup_kittest_for_rendering_ui(size)
        .build_ui(move |ui| test_context.run_with_single_view(ui, view_id));
    harness.run();
    harness
}

/// Render the view's selection-panel sections with these titles, under a "Done" button that
/// takes focus away from them.
fn selection_harness<'a>(
    test_context: &'a TestContext,
    view_id: ViewId,
    origin: &'a EntityPath,
    titles: &'a [&'a str],
    size: egui::Vec2,
) -> Harness<'a> {
    let mut harness = test_context
        .setup_kittest_for_rendering_ui(size)
        .build_ui(move |ui| {
            test_context.run_ui(ui, |ctx, ui| {
                ui.button("Done").clicked();

                let state = RawDataView.new_state();
                let view_ctx = RawDataView.view_context(ctx, view_id, state.as_ref(), origin);
                let selection = RawDataView.selection_ui(&view_ctx);
                for title in titles {
                    let section = selection
                        .extra_sections
                        .iter()
                        .find(|section| section.title == *title)
                        .unwrap_or_else(|| panic!("raw data view has no {title:?} section"));
                    (section.ui)(ui, &view_ctx);
                }
            });
            test_context.handle_system_commands(ui.ctx());
        });
    harness.run();
    harness
}

/// Which of the example components the view shows.
fn shown_components(harness: &Harness<'_>) -> Vec<&'static str> {
    ["colors", "points"]
        .into_iter()
        .filter(|name| harness.query_by_label(name).is_some())
        .collect()
}

fn alt_click_first_checkbox(harness: &mut Harness<'_>) {
    harness
        .get_all_by_role(egui::accesskit::Role::CheckBox)
        .next()
        .expect("no component checkbox")
        .click_modifiers(egui::Modifiers::ALT);
    harness.run();
}

/// A deeply nested message, of the shape an MCAP/ROS recording produces.
fn nested_message_context(copies: usize) -> (TestContext, EntityPath) {
    use arrow::array::{Array as _, ArrayRef, Float64Array, Int32Array, StringArray, StructArray};
    use arrow::datatypes::{Field, Fields};

    fn struct_array(fields: Vec<(&str, ArrayRef)>) -> StructArray {
        let (fields, arrays): (Vec<_>, Vec<_>) = fields
            .into_iter()
            .map(|(name, array)| (Field::new(name, array.data_type().clone(), false), array))
            .unzip();
        StructArray::new(Fields::from(fields), arrays, None)
    }

    let stamp = struct_array(vec![
        ("sec", Arc::new(Int32Array::from(vec![924]))),
        ("nanosec", Arc::new(Int32Array::from(vec![102_000_000]))),
    ]);
    let header = struct_array(vec![
        ("stamp", Arc::new(stamp)),
        ("frame_id", Arc::new(StringArray::from(vec!["map"]))),
    ]);
    let position = struct_array(vec![
        (
            "x",
            Arc::new(Float64Array::from(vec![4.365_196_653_967_71])),
        ),
        ("y", Arc::new(Float64Array::from(vec![7.0]))),
        ("z", Arc::new(Float64Array::from(vec![0.0]))),
    ]);
    let message = struct_array(vec![
        ("header", Arc::new(header)),
        ("position", Arc::new(position)),
    ]);

    let entity_path = EntityPath::from("amcl_pose");
    let mut test_context = TestContext::new_with_view_class::<RawDataView>();
    let timeline = frame_timeline();

    let message: ArrayRef = Arc::new(message);
    for _ in 0..copies {
        test_context.log_entity(entity_path.clone(), |builder| {
            builder.with_row(
                RowId::new(),
                TimePoint::from([(timeline, 1)]),
                [(
                    re_sdk_types::ComponentDescriptor::partial("message"),
                    message.clone(),
                )],
            )
        });
    }

    test_context.set_active_timeline(*timeline.name());
    (test_context, entity_path)
}

// ---------------------------------------------------------------------------------------------
// View rendering and configuration

/// Without configuration the view shows every component of the entity, grouped under a single
/// archetype heading.
#[test]
fn test_raw_data_shows_all_components() {
    let mut test_context = test_context();
    let view_id = setup_default_view(&mut test_context);
    test_context.set_time(30);

    test_context
        .run_view_ui_and_save_snapshot(
            view_id,
            "raw_data_all_components",
            egui::vec2(400.0, 300.0),
            None,
        )
        .unwrap();
}

/// Components not listed in the blueprint are hidden.
#[test]
fn test_configured_rows_hide_the_rest() {
    let mut test_context = test_context();
    let view_id = setup_view_with_rows(&mut test_context, &[colors()]);
    test_context.set_time(30);

    let harness = view_harness(&test_context, view_id);

    assert_eq!(shown_components(&harness), ["colors"]);
}

/// The sole row of a view is what the reader came for, so a nested one opens by itself.
/// An open row drops its summary, a shut one keeps it.
#[test]
fn test_sole_nested_component_opens_by_default() {
    let (mut test_context, entity_path) = nested_message_context(1);
    let view_id = setup_view(
        &mut test_context,
        RecommendedView::new_single_entity(entity_path),
        |_, _| {},
    );

    let mut harness = view_harness(&test_context, view_id);

    for field in ["header", "position"] {
        assert!(
            harness.query_by_label(field).is_some(),
            "a sole nested component should open by itself and reveal `{field}`"
        );
    }
    assert!(
        harness.query_by_label_contains("{header:").is_none(),
        "an expanded row should drop its summary"
    );
    harness
        .get_by_label("{stamp: {sec: 924, nanosec: 102\u{2009}000\u{2009}000}, frame_id: \"map\"}");

    harness.snapshot("nested_message_expanded");
}

/// A component with several instances opens onto one indexed row per instance.
#[test]
fn test_multi_instance_component_reads_as_a_list() {
    let mut test_context = TestContext::new_with_view_class::<RawDataView>();
    let timeline = frame_timeline();
    test_context.log_entity(ENTITY, |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::from([(timeline, 1)]),
            &MyPoints::new([
                MyPoint::new(0.0, 1.0),
                MyPoint::new(2.0, 3.0),
                MyPoint::new(4.0, 5.0),
            ]),
        )
    });
    test_context.set_active_timeline(*timeline.name());
    let view_id = setup_default_view(&mut test_context);

    let mut harness = view_harness(&test_context, view_id);

    for index in ["0", "1", "2"] {
        assert!(
            harness.query_by_label(index).is_some(),
            "each instance should be a row of its own, labelled `{index}`"
        );
    }

    harness.snapshot("raw_data_multi_instance");
}

/// Compact drops the archetype headings and names each row in full instead.
#[test]
fn test_compact_style() {
    let mut test_context = test_context();
    let view_id = setup_view_with_property::<RawDataViewFormat>(
        &mut test_context,
        &RawDataViewFormat::descriptor_style(),
        &[blueprint::components::ViewDensity::Compact],
    );
    test_context.set_time(30);

    let mut harness = view_harness_with_size(&test_context, view_id, egui::vec2(400.0, 300.0));

    let archetype = MyPoints::descriptor_points()
        .archetype
        .expect("the example archetype has a name");
    assert!(
        harness.query_by_label(archetype.short_name()).is_none(),
        "compact should drop the archetype heading"
    );
    let points = MyPoints::descriptor_points().component;
    assert!(
        harness.query_by_label(points.as_str()).is_some(),
        "compact should name each row `{points}`, archetype and all"
    );

    harness.snapshot("raw_data_compact");
}

/// Hiding the timestamp takes the whole navigation bar with it, arrows included.
#[test]
fn test_hidden_timestamp_removes_the_navigation_bar() {
    let mut test_context = test_context();
    let view_id = setup_view_with_property::<RawDataViewFormat>(
        &mut test_context,
        &RawDataViewFormat::descriptor_show_time_navigation(),
        &[Visible::from(false)],
    );
    test_context.set_time(30);

    let harness = view_harness(&test_context, view_id);

    for button in ["Previous event", "Next event"] {
        assert!(
            harness.query_by_label(button).is_none(),
            "{button:?} should go with the timestamp"
        );
    }
    assert_eq!(shown_components(&harness), ["colors", "points"]);
}

// ---------------------------------------------------------------------------------------------
// View interactions

/// With more than one row the list stays scannable: rows are collapsed, nested ones expand on
/// request, and a plain scalar has nothing to expand.
#[test]
fn test_several_rows_stay_collapsed_until_expanded() {
    let mut test_context = test_context();
    let view_id = setup_default_view(&mut test_context);
    test_context.set_time(30);

    let mut harness = view_harness(&test_context, view_id);

    assert!(
        harness.query_by_label("x").is_none(),
        "rows should stay collapsed when there is more than one"
    );

    let points_y = harness.get_by_label("points").rect().top();
    harness.get_by_label("colors").click();
    harness.run();
    assert_eq!(
        harness.get_by_label("points").rect().top(),
        points_y,
        "the scalar `colors` row above `points` should not open"
    );

    harness.get_by_label("points").click();
    harness.run();
    for field in ["x", "y"] {
        assert!(
            harness.query_by_label(field).is_some(),
            "expanding `points` should reveal `{field}`"
        );
    }
}

/// A click anywhere on a row opens it, not only one on the chevron.
#[test]
fn test_row_click_toggles() {
    let mut test_context = test_context();
    let view_id = setup_default_view(&mut test_context);
    test_context.set_time(30);

    let mut harness = view_harness(&test_context, view_id);
    assert!(harness.query_by_label("x").is_none(), "starts shut");

    for expect_open in [true, false] {
        // The label sits well clear of the chevron at the row's left edge.
        harness.get_by_label("points").click();
        harness.run();

        assert_eq!(
            harness.query_by_label("x").is_some(),
            expect_open,
            "clicking the row should toggle it"
        );
    }
}

/// Every row selects its component, whether or not it has anything to expand.
#[test]
fn test_clicking_a_scalar_row_selects_the_component() {
    let mut test_context = test_context();
    let view_id = setup_default_view(&mut test_context);
    test_context.set_time(30);

    let mut harness = view_harness(&test_context, view_id);
    harness.get_by_label("colors").click();
    // The selection is applied through a system command, which lands on the next frame.
    harness.run();
    drop(harness);

    assert_eq!(
        test_context
            .selection_state
            .lock()
            .selected_items()
            .single_item()
            .cloned(),
        Some(re_viewer_context::Item::ComponentPath(
            re_log_types::ComponentPath::new(EntityPath::from(ENTITY), colors())
        )),
    );
}

/// Values logged onto the same time point are listed separately, and each opens on its own.
#[test]
fn test_values_on_one_time_point_expand_independently() {
    let (mut test_context, entity_path) = nested_message_context(2);
    let view_id = setup_view(
        &mut test_context,
        RecommendedView::new_single_entity(entity_path),
        |_, _| {},
    );

    let mut harness = view_harness(&test_context, view_id);
    assert_eq!(harness.query_all_by_label("header").count(), 2);
    assert_eq!(harness.query_all_by_label("stamp").count(), 0);

    harness.get_all_by_label("header").next().unwrap().click();
    harness.run();

    assert_eq!(
        harness.query_all_by_label("stamp").count(),
        1,
        "only the clicked value should open"
    );
}

/// Ensure we can copy out arrow-ui values easily here. That's a general property, but it's particularly important for the raw view!
#[test]
fn test_nested_value_can_be_selected_and_copied() {
    let (mut test_context, entity_path) = nested_message_context(1);
    let view_id = setup_view(
        &mut test_context,
        RecommendedView::new_single_entity(entity_path),
        |_, _| {},
    );
    let mut harness = view_harness(&test_context, view_id);
    harness.get_by_label("header").click();
    harness.run();
    harness.get_by_label("stamp").click();
    harness.run();

    let value = harness.get_by_label("924").rect();
    harness.drag_at(value.left_center());
    harness.step();
    harness.hover_at(value.right_center());
    harness.step();
    harness.drop_at(value.right_center());
    harness.step();
    harness.event(egui::Event::Copy);
    harness.step();

    assert!(
        harness
            .output()
            .platform_output
            .commands
            .iter()
            .any(|command| {
                matches!(command, egui::OutputCommand::CopyText(text) if text == "924")
            })
    );
    assert!(harness.query_by_label("sec").is_some());
}

/// Stepping forward lands on the next event of the rows on show.
///
/// With only the points row visible, pressing next at frame 10 must skip the color event at 20.
#[test]
fn test_next_event_skips_hidden_components() {
    let mut test_context = test_context();
    let view_id = setup_view_with_rows(&mut test_context, &[points()]);
    test_context.set_time(10);

    let mut harness = view_harness(&test_context, view_id);
    harness.get_by_label("Next event").click();
    harness.run();
    drop(harness);

    assert_eq!(
        test_context.time_ctrl.read().time_int(),
        Some(re_log_types::TimeInt::new_temporal(30)),
    );
}

#[test]
fn test_previous_event_steps_from_displayed_event() {
    let mut test_context = test_context();
    let view_id = setup_view_with_rows(&mut test_context, &[points()]);
    test_context.set_time(40);

    let mut harness = view_harness(&test_context, view_id);
    assert!(harness.query_by_label("#30").is_some());
    assert!(harness.query_by_label("#40").is_none());
    harness.get_by_label("Previous event").click();
    harness.run();
    drop(harness);

    assert_eq!(
        test_context.time_ctrl.read().time_int(),
        Some(re_log_types::TimeInt::new_temporal(10)),
    );
}

#[test]
fn test_next_event_before_first_event() {
    let mut test_context = test_context();
    let view_id = setup_view_with_rows(&mut test_context, &[points()]);
    test_context.set_time(0);

    let mut harness = view_harness(&test_context, view_id);
    assert!(harness.query_by_label("No event").is_some());
    harness.get_by_label("Next event").click();
    harness.run();
    drop(harness);

    assert_eq!(
        test_context.time_ctrl.read().time_int(),
        Some(re_log_types::TimeInt::new_temporal(10)),
    );
}

// ---------------------------------------------------------------------------------------------
// Selection panel

/// The panel lists every component of the entity by short name, grouped under its archetype.
#[test]
fn test_property_panel_lists_components_by_archetype() {
    let mut test_context = test_context();
    let view_id = setup_default_view(&mut test_context);
    test_context.set_time(30);
    let origin = EntityPath::from(ENTITY);

    let mut harness = selection_harness(
        &test_context,
        view_id,
        &origin,
        &["Components"],
        egui::vec2(400.0, 500.0),
    );

    let archetype = MyPoints::descriptor_points()
        .archetype
        .expect("the example archetype has a name");
    assert!(
        harness.query_by_label(archetype.short_name()).is_some(),
        "the archetype should head the group"
    );
    assert_eq!(shown_components(&harness), ["colors", "points"]);
    assert_eq!(
        harness
            .get_all_by_role(egui::accesskit::Role::CheckBox)
            .count(),
        2,
        "one tick box per component"
    );

    harness.snapshot("property_panel_checklist");
}

#[test]
fn test_unticking_a_component_hides_it() {
    let mut test_context = test_context();
    let view_id = setup_default_view(&mut test_context);
    test_context.set_time(30);
    let origin = EntityPath::from(ENTITY);

    let mut harness = selection_harness(
        &test_context,
        view_id,
        &origin,
        &["Components"],
        egui::vec2(400.0, 300.0),
    );
    harness
        .get_all_by_role(egui::accesskit::Role::CheckBox)
        .next()
        .expect("no component checkbox")
        .click();
    harness.run();
    drop(harness);

    // The first tick box is `colors`.
    assert_eq!(
        shown_components(&view_harness(&test_context, view_id)),
        ["points"]
    );
}

/// Unticking the last ticked box would empty the list, which reads back as "show everything".
#[test]
fn test_last_ticked_component_cannot_be_unticked() {
    let mut test_context = test_context();
    let view_id = setup_view_with_rows(&mut test_context, &[points()]);
    test_context.set_time(30);
    let origin = EntityPath::from(ENTITY);

    let mut harness = selection_harness(
        &test_context,
        view_id,
        &origin,
        &["Components"],
        egui::vec2(400.0, 300.0),
    );
    // The second tick box is `points`, the only one ticked.
    harness
        .get_all_by_role(egui::accesskit::Role::CheckBox)
        .nth(1)
        .expect("no points checkbox")
        .click();
    harness.run();
    drop(harness);

    assert_eq!(
        shown_components(&view_harness(&test_context, view_id)),
        ["points"]
    );
}

/// Option-clicking a tick box shows only that component, and doing it again shows them all.
#[test]
fn test_option_click_solos_and_restores() {
    let mut test_context = test_context();
    let view_id = setup_default_view(&mut test_context);
    test_context.set_time(30);
    let origin = EntityPath::from(ENTITY);

    let mut harness = selection_harness(
        &test_context,
        view_id,
        &origin,
        &["Components"],
        egui::vec2(400.0, 300.0),
    );
    alt_click_first_checkbox(&mut harness);
    drop(harness);

    assert_eq!(
        shown_components(&view_harness(&test_context, view_id)),
        ["colors"]
    );

    let mut harness = selection_harness(
        &test_context,
        view_id,
        &origin,
        &["Components"],
        egui::vec2(400.0, 300.0),
    );
    alt_click_first_checkbox(&mut harness);
    drop(harness);

    assert_eq!(
        shown_components(&view_harness(&test_context, view_id)),
        ["colors", "points"]
    );
}

/// Choosing another entity in the Entity field shows it, with all its components.
#[test]
fn test_choosing_an_entity_resets_the_checklist() {
    let mut test_context = test_context();
    log_text(&mut test_context, "other");
    let view_id = setup_view_with_rows(&mut test_context, &[points()]);
    test_context.set_time(30);
    let origin = EntityPath::from(ENTITY);

    // The selection UI reads the view's query result, which only exists once the view has run.
    drop(view_harness(&test_context, view_id));

    let mut harness = selection_harness(
        &test_context,
        view_id,
        &origin,
        &["Entity"],
        egui::vec2(400.0, 300.0),
    );
    harness
        .get_by_role(egui::accesskit::Role::TextInput)
        .click();
    harness.run();
    harness
        .get_by_role(egui::accesskit::Role::TextInput)
        .type_text("/other");
    harness.run();
    harness.get_by_label("Done").click();
    harness.run();
    drop(harness);

    // The test harness caches view query results until the blueprint is reloaded.
    test_context.setup_viewport_blueprint(|_, _| {});

    let view = view_harness(&test_context, view_id);
    assert!(
        view.query_by_label("points").is_none(),
        "the view should display the newly entered entity"
    );
    assert!(
        view.query_by_label("text").is_some(),
        "the old entity's checklist should not hide the new entity's components"
    );
}

/// The view holds one entity but the filter can match many: it takes the first in the hierarchy
/// that has data, regardless of the origin, and the Entity field says so.
#[test]
fn test_filter_matching_several_entities_shows_the_first() {
    let mut test_context = test_context();
    test_context.log_entity("zzz_other", |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::from([(frame_timeline(), 10)]),
            &MyPoints::new([MyPoint::new(0.0, 0.0)]),
        )
    });

    // `/**` also matches `/`, which has no data of its own.
    let origin = EntityPath::from("zzz_other");
    let view_id = setup_view(
        &mut test_context,
        RecommendedView {
            origin: origin.clone(),
            query_filter: "/**".parse().expect("invalid filter"),
        },
        |_, _| {},
    );

    // The note counts what the view's query matched, which only exists once the view has run.
    drop(view_harness(&test_context, view_id));

    let mut harness = selection_harness(
        &test_context,
        view_id,
        &origin,
        &["Entity"],
        egui::vec2(400.0, 200.0),
    );

    let note = harness
        .query_by_label_contains("can only display")
        .expect("the entity field should explain why only one entity is shown");
    assert_eq!(
        note.value().as_deref(),
        Some("View can only display 1 entity, currently displays /points"),
    );

    harness.snapshot("raw_data_entity_field_note");
}
