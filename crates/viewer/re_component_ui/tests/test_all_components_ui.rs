#![allow(clippy::unwrap_used)]

use std::collections::HashSet;
use std::fmt::Formatter;
use std::fs;

use arrow::array::ArrayRef;
use egui::Vec2;
use egui_kittest::{SnapshotError, SnapshotOptions};
use itertools::Itertools;
use nohash_hasher::IntSet;

use re_component_ui::create_component_ui_registry;
use re_log_types::{EntityPath, Timeline};
use re_types::{
    blueprint::components::{ComponentColumnSelector, QueryExpression},
    components::{self, GraphEdge, GraphNode, ImageFormat, RecordingUri, Text},
    datatypes::{ChannelDatatype, PixelFormat},
};
use re_types_core::{reflection::Reflection, Component, ComponentName, LoggableBatch};
use re_ui::{list_item, UiExt};
use re_viewer_context::{
    external::re_chunk_store::{external::re_chunk, LatestAtQuery},
    test_context::TestContext,
    UiLayout, ViewerContext,
};

/// Test case master list.
///
/// Edit this function to fine-tune the list of test cases. By default, every component in the
/// [`Reflection`] will be added to the list using their placeholder content. You can both exclude
/// components from that list and add test cases with custom component values.
fn test_cases(reflection: &Reflection) -> Vec<TestCase> {
    //
    // ADD YOUR CUSTOM TEST CASES HERE!
    //

    let custom_test_cases = [
        TestCase::from_component(
            ComponentColumnSelector::new(
                &EntityPath::from("/world"),
                "rerun.components.Position3D",
            ),
            "simple",
        ),
        TestCase::from_component(
            components::EntityPath::from("/world/robot/camera"),
            "simple",
        ),
        TestCase::from_component(GraphNode::from("graph_node"), "simple"),
        TestCase::from_component(GraphEdge::from(("node_a", "node_b")), "simple"),
        TestCase::from_component(ImageFormat::rgb8([640, 480]), "rgb8"),
        TestCase::from_component(ImageFormat::rgba8([640, 480]), "rgba8"),
        TestCase::from_component(
            ImageFormat::depth([640, 480], ChannelDatatype::F32),
            "depth_f32",
        ),
        TestCase::from_component(
            ImageFormat::segmentation([640, 480], ChannelDatatype::U32),
            "segmentation_u32",
        ),
        TestCase::from_component(
            ImageFormat::from_pixel_format([640, 480], PixelFormat::NV12),
            "nv12",
        ),
        TestCase::from_component(QueryExpression::from("+ /world/**"), "simple"),
        TestCase::from_component(
            RecordingUri::from("rerun://0.0.0.0:1234/recordings/XYZ"),
            "simple",
        ),
        TestCase::from_component(Text::from("Hello World!"), "simple"),
    ];

    //
    // EXCLUDE COMPONENTS FROM THE PLACEHOLDER LIST HERE!
    //

    let excluded_components = [
        // TODO(#6661): these components still have special treatment via `DataUi` and
        // `EntityDatatUi`. The hooks are registered by `re_data_ui::register_component_uis`, which
        // is not available here. So basically no point testing them here.
        re_types::components::AnnotationContext::name(),
        re_types::components::Blob::name(),
        re_types::components::ClassId::name(),
        re_types::components::ImageBuffer::name(), // this one is not technically handled by `DataUi`, but should get a custom ui first (it's using default ui right now).
        re_types::components::KeypointId::name(),
        re_types::components::TensorData::name(),
        //
        // no need to clutter the tests with these internal blueprint types
        re_types::blueprint::components::ActiveTab::name(),
        re_types::blueprint::components::AutoLayout::name(),
        re_types::blueprint::components::AutoViews::name(),
        re_types::blueprint::components::ColumnShare::name(),
        re_types::blueprint::components::IncludedContent::name(),
        re_types::blueprint::components::PanelState::name(),
        re_types::blueprint::components::RootContainer::name(),
        re_types::blueprint::components::RowShare::name(),
        re_types::blueprint::components::ViewMaximized::name(),
        re_types::blueprint::components::ViewOrigin::name(),
        re_types::blueprint::components::ViewerRecommendationHash::name(),
        re_types::blueprint::components::VisualizerOverrides::name(),
    ]
    .into_iter()
    // Exclude components that have custom test cases.
    .chain(
        custom_test_cases
            .iter()
            .map(|test_case| test_case.component_name),
    )
    .collect::<IntSet<_>>();

    //
    // Placeholder test cases for all components.
    //

    let placeholder_test_cases = reflection
        .components
        .keys()
        .filter(|component_name| !excluded_components.contains(*component_name))
        .map(|&component_name| {
            let component_data = placeholder_for_component(reflection, component_name).unwrap();
            TestCase {
                label: "placeholder",
                component_name,
                component_data,
            }
        });

    placeholder_test_cases
        .chain(custom_test_cases)
        .sorted_by(|left, right| {
            left.component_name
                .short_name()
                .cmp(right.component_name.short_name())
                .then_with(|| left.label.cmp(right.label))
        })
        .collect_vec()
}

// ---

/// Test all components UI in a narrow list item context.
#[test]
pub fn test_all_components_ui_as_list_items_narrow() {
    let test_context = get_test_context();
    let test_cases = test_cases(&test_context.reflection);
    let snapshot_options =
        SnapshotOptions::default().output_path("tests/snapshots/all_components_list_item_narrow");

    let results = test_cases
        .iter()
        .map(|test_case| {
            test_single_component_ui_as_list_item(
                &test_context,
                test_case,
                200.0,
                &snapshot_options,
            )
        })
        .collect_vec();

    check_for_unused_snapshots(&test_cases, &snapshot_options);
    check_and_print_results(&test_cases, &results);
}

/// Test all components UI in a wide list item context.
#[test]
pub fn test_all_components_ui_as_list_items_wide() {
    let test_context = get_test_context();
    let test_cases = test_cases(&test_context.reflection);
    let snapshot_options =
        SnapshotOptions::default().output_path("tests/snapshots/all_components_list_item_wide");

    let results = test_cases
        .iter()
        .map(|test_case| {
            test_single_component_ui_as_list_item(
                &test_context,
                test_case,
                600.0,
                &snapshot_options,
            )
        })
        .collect_vec();

    check_for_unused_snapshots(&test_cases, &snapshot_options);
    check_and_print_results(&test_cases, &results);
}

fn test_single_component_ui_as_list_item(
    test_context: &TestContext,
    test_case: &TestCase,
    ui_width: f32,
    _snapshot_options: &SnapshotOptions,
) -> Result<(), SnapshotError> {
    let actual_ui = |ctx: &ViewerContext<'_>, ui: &mut egui::Ui| {
        ui.list_item_flat_noninteractive(
            list_item::PropertyContent::new("ComponentName").value_fn(|ui, _| {
                ctx.component_ui_registry.ui_raw(
                    ctx,
                    ui,
                    UiLayout::List,
                    // Note: recording and queries are only used for tooltips,
                    // which we are not testing here.
                    &LatestAtQuery::latest(Timeline::log_time()),
                    ctx.recording(),
                    &EntityPath::root(),
                    test_case.component_name,
                    None,
                    &*test_case.component_data,
                );
            }),
        );
    };

    let mut harness = egui_kittest::Harness::builder()
        .with_size(Vec2::new(ui_width, 40.0))
        .build_ui(|ui| {
            test_context.run(&ui.ctx().clone(), |ctx| {
                ui.full_span_scope(ui.max_rect().x_range(), |ui| {
                    list_item::list_item_scope(ui, "list_item_scope", |ui| {
                        actual_ui(ctx, ui);
                    });
                });
            });
        });

    harness.run();

    //TODO(#8245): enable this everywhere when we have a software renderer setup
    if cfg!(target_os = "macos") {
        harness.try_wgpu_snapshot_options(&format!("{test_case}"), _snapshot_options)
    } else {
        Ok(())
    }
}

// ---

/// Description of a single test case.
struct TestCase {
    /// Label for the test case.
    ///
    /// Labels must be unique per component.
    label: &'static str,

    /// The component this test case refers to.
    component_name: ComponentName,

    /// The data for that component.
    component_data: ArrayRef,
}

impl std::fmt::Display for TestCase {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}_{}", self.component_name.short_name(), self.label)
    }
}

impl TestCase {
    #[allow(clippy::needless_pass_by_value)]
    fn from_component<C: Component>(component: C, label: &'static str) -> Self {
        let component_name = C::name();
        let component_data = LoggableBatch::to_arrow(&component).unwrap();
        Self {
            label,
            component_name,
            component_data,
        }
    }
}

/// Ensures that we don't have a dangling snapshot image that is no longer used.
///
/// This assumes that each snapshot image is named after `TestCase` display impl.
fn check_for_unused_snapshots(test_cases: &[TestCase], snapshot_options: &SnapshotOptions) {
    let ok_file_names = test_cases
        .iter()
        .map(ToString::to_string)
        .collect::<HashSet<_>>();

    for entry in fs::read_dir(&snapshot_options.output_path).unwrap() {
        let path = entry.unwrap().path();

        if !path.is_file() {
            continue;
        }

        let file_name = path.file_name().unwrap().to_string_lossy().to_string();

        if file_name.ends_with(".png")
            && !file_name.ends_with(".new.png")
            && !file_name.ends_with(".diff.png")
            && !ok_file_names.contains(file_name.strip_suffix(".png").unwrap())
        {
            panic!(
                "File {} does not belong to any known test",
                path.to_string_lossy()
            )
        }
    }
}

/// Pretty prints a list of test cases with the OK/NOK result and panics if any of the tests failed.
fn check_and_print_results(test_cases: &[TestCase], results: &[Result<(), SnapshotError>]) {
    let component_name_width = test_cases
        .iter()
        .map(|test_case| test_case.component_name.short_name().len())
        .max()
        .unwrap();

    let label_width = test_cases
        .iter()
        .map(|test_case| test_case.label.len())
        .max()
        .unwrap();

    for (test_case, result) in test_cases.iter().zip(results.iter()) {
        match result {
            Ok(_) => println!(
                "{:>component_name_width$}[{:label_width$}] OK",
                test_case.component_name.short_name(),
                test_case.label,
            ),
            Err(err) => println!(
                "{:>component_name_width$}[{:label_width$}] ERR {}",
                test_case.component_name.short_name(),
                test_case.label,
                err,
            ),
        }
    }

    assert!(
        results.iter().all(Result::is_ok),
        "Some test cases failed, see previous output."
    );
}

/// Create a [`TestContext`] with a fully populated component ui registry.
// TODO(ab): It would be nice to generalise this utility. However, TestContext current lives in
// re_viewer_context, which cannot depend on re_component_ui.
fn get_test_context() -> TestContext {
    let mut test_context = TestContext::default();
    test_context.component_ui_registry = create_component_ui_registry();
    test_context
}

/// Get some placeholder data for the provided component.
///
/// This is a simpler version of [`ViewerContext::placeholder_for`] which doesn't attempt to infer
/// datatypes from store contents. As a result, it will fail for user-defined components, which is
/// fine as we only test built-in components here.
fn placeholder_for_component(
    reflection: &Reflection,
    component: re_chunk::ComponentName,
) -> Option<ArrayRef> {
    let datatype = if let Some(reflection) = reflection.components.get(&component) {
        if let Some(placeholder) = reflection.custom_placeholder.as_ref() {
            return Some(placeholder.clone());
        }
        Some(reflection.datatype.clone())
    } else {
        None
    };

    datatype
        .map(|datatype| re_types::reflection::generic_placeholder_for_datatype(&datatype).into())
}
