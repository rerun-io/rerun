use arrow::datatypes::{DataType, Field};
use egui_kittest::kittest::Queryable as _;
use egui_kittest::{Harness, HarnessBuilder};
use re_log_types::Timeline;
use re_sorbet::{BatchType, IndexColumnDescriptor, RowIdColumnDescriptor};

use super::*;

fn component(path: &str, archetype: &str, name: &str, datatype: DataType) -> Field {
    let archetype =
        re_log_types::external::re_types_core::ArchetypeName::try_new(archetype).unwrap();
    ComponentColumnDescriptor {
        entity_path: path.into(),
        archetype: Some(archetype),
        component: re_log_types::external::re_types_core::ComponentIdentifier::from_archetype_field(
            archetype, name,
        ),
        component_type: None,
        store_datatype: DataType::List(Arc::new(Field::new("item", datatype, true))),
        is_static: false,
        is_tombstone: false,
        is_semantically_empty: false,
    }
    .to_arrow_field(BatchType::Dataframe)
}

fn static_component(path: &str, archetype: &str, name: &str, datatype: DataType) -> Field {
    let field = component(path, archetype, name, datatype);
    let mut descriptor = ComponentColumnDescriptor::from_arrow_field(None, &field).unwrap();
    descriptor.is_static = true;
    descriptor.to_arrow_field(BatchType::Dataframe)
}

fn schema_from_fields(fields: Vec<Field>) -> DatasetSchema {
    DatasetSchema::from_arrow(&Schema::new_with_metadata(fields, Default::default())).unwrap()
}

fn example_schema() -> DatasetSchema {
    let vector = DataType::FixedSizeList(Arc::new(Field::new("item", DataType::Float32, false)), 3);
    schema_from_fields(vec![
        component(
            "/cameras/ext1",
            "rerun.archetypes.EncodedImage",
            "blob",
            DataType::Binary,
        ),
        component(
            "/cameras/ext1",
            "rerun.archetypes.EncodedImage",
            "media_type",
            DataType::Utf8,
        ),
        static_component(
            "/cameras/ext1",
            "rerun.archetypes.Pinhole",
            "image_from_camera",
            DataType::FixedSizeList(Arc::new(Field::new("item", DataType::Float32, false)), 9),
        ),
        static_component(
            "/cameras/ext1",
            "rerun.archetypes.Pinhole",
            "resolution",
            DataType::FixedSizeList(Arc::new(Field::new("item", DataType::Float32, false)), 2),
        ),
        component(
            "/cameras/ext1",
            "rerun.archetypes.Transform3D",
            "translation",
            vector.clone(),
        ),
        component(
            "/cameras/ext2",
            "rerun.archetypes.EncodedImage",
            "blob",
            DataType::Binary,
        ),
        component(
            "/cameras/wrist",
            "rerun.archetypes.EncodedImage",
            "blob",
            DataType::Binary,
        ),
        component(
            "/robot/joint_states",
            "rerun.archetypes.Scalars",
            "scalars",
            DataType::Float64,
        ),
        component(
            "/robot/tcp",
            "rerun.archetypes.Transform3D",
            "translation",
            vector.clone(),
        ),
        component(
            "/scene/points",
            "rerun.archetypes.Points3D",
            "positions",
            vector,
        ),
        component(
            "/scene/points",
            "rerun.archetypes.Points3D",
            "colors",
            DataType::UInt32,
        ),
        IndexColumnDescriptor::from_timeline(Timeline::new_sequence("frame"), false)
            .to_arrow_field(),
        IndexColumnDescriptor::from_timeline(Timeline::log_time(), false).to_arrow_field(),
        RowIdColumnDescriptor::from_sorted(false).to_arrow_field(),
    ])
}

fn schema_harness(schema: DatasetSchema, size: impl Into<egui::Vec2>) -> Harness<'static> {
    schema_harness_with(
        re_ui::testing::new_harness(re_ui::testing::TestOptions::Gui, size),
        schema,
    )
}

fn schema_harness_with(builder: HarnessBuilder, schema: DatasetSchema) -> Harness<'static> {
    let schema = ServerValue::Completed(Arc::new(schema));
    let mut harness = builder.build_ui(move |ui| {
        re_ui::apply_style_and_install_loaders(ui.ctx());
        schema_ui(ui, &schema);
    });
    harness.run();
    harness
}

/// Scrolls the scroll area under `pos` by `delta`, then moves the cursor away.
fn scroll(harness: &mut Harness<'_>, pos: egui::Pos2, delta: egui::Vec2) {
    harness.hover_at(pos);
    harness.event(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta,
        phase: egui::TouchPhase::Move,
        modifiers: egui::Modifiers::NONE,
        source: egui::MouseWheelSource::Unknown,
    });
    harness.step();
    harness.remove_cursor();
    harness.run();
}

/// Turns on the metadata column through the display menu.
fn show_metadata_column(harness: &mut Harness<'_>) {
    harness.get_by_label("Display").click();
    harness.run();
    harness.get_by_label("Metadata").click();
    harness.run();
    harness.key_press(egui::Key::Escape);
    harness.run();
}

/// Components from a merged schema are grouped by entity, with their parent paths kept.
/// Row IDs and timelines are kept out of the component tree.
/// The entity count covers only the entities that have components.
#[test]
fn builds_entity_tree() {
    let schema = example_schema();

    assert_eq!(schema.component_count, 11);
    assert_eq!(schema.entities.len(), 10);
    assert_eq!(schema.entity_count, 6);
    assert_eq!(schema.entities[&EntityPath::root()].children.len(), 3);

    assert!(
        schema.entities[&EntityPath::from("/cameras")]
            .components
            .is_empty()
    );
    assert_eq!(
        schema.entities[&EntityPath::from("/cameras/ext1")]
            .components
            .len(),
        5
    );
    assert_eq!(
        schema.entities[&EntityPath::from("/scene/points")].components[0]
            .descriptor
            .display_name(),
        "Points3D:colors"
    );
}

/// Entity filtering is case-insensitive and keeps the parents of matching entities.
#[test]
#[expect(clippy::single_range_in_vec_init)]
fn filters_entity_paths() {
    let schema = example_schema();
    let mut filter = FilterState::default();
    assert_eq!(
        schema.visible_entities(&filter).len(),
        schema.entities.len()
    );

    filter.activate("CAMERAS/ext1");
    assert_eq!(
        schema.visible_entities(&filter),
        BTreeMap::from([
            (EntityPath::root(), Vec::new()),
            (EntityPath::from("/cameras"), vec![0..7]),
            (EntityPath::from("/cameras/ext1"), vec![0..4]),
        ])
    );

    filter.activate("missing");
    assert!(schema.visible_entities(&filter).is_empty());
}

/// Path matches highlight the matching text in both parent and leaf labels.
/// Overlapping keyword matches are merged before rendering.
#[test]
fn highlights_entity_path_parts() {
    let schema = example_schema();
    let mut filter = FilterState::default();

    filter.activate("RAS/wr");
    let visible = schema.visible_entities(&filter);
    assert_eq!(visible[&EntityPath::from("/cameras")], vec![4..7]);
    assert_eq!(visible[&EntityPath::from("/cameras/wrist")], vec![0..2]);

    filter.activate("ca cam");
    let visible = schema.visible_entities(&filter);
    assert_eq!(visible[&EntityPath::from("/cameras")], vec![0..3]);
}

/// An empty schema has no entities or components.
#[test]
fn empty_schema() {
    let schema = DatasetSchema::from_arrow(&Schema::empty()).unwrap();

    assert!(schema.entities.is_empty());
    assert_eq!(schema.entity_count, 0);
    assert_eq!(schema.component_count, 0);
}

/// Components logged at the root appear on the root entity.
#[test]
fn root_components() {
    let schema = schema_from_fields(vec![component(
        "/",
        "rerun.archetypes.Scalars",
        "scalars",
        DataType::Float64,
    )]);

    assert_eq!(schema.entities.len(), 1);
    assert_eq!(schema.entity_count, 1);
    assert_eq!(schema.entities[&EntityPath::root()].components.len(), 1);
}

/// Invalid column metadata gives an error.
#[test]
fn invalid_schema() {
    let schema = Schema::new_with_metadata(
        vec![
            Field::new("unknown", DataType::Boolean, true).with_metadata(
                std::collections::HashMap::from([(
                    re_sorbet::RERUN_KIND.to_owned(),
                    "unknown".to_owned(),
                )]),
            ),
        ],
        Default::default(),
    );

    assert!(DatasetSchema::from_arrow(&schema).is_err());
}

/// Short entity rows keep their counts visible when another row makes the tree wider.
/// The counts stay aligned to the viewport while scrolling horizontally.
#[test]
fn schema_page_tree_counts() {
    let schema = schema_from_fields(vec![
        component(
            "/long_entity_path_that_makes_the_tree_wider_than_its_viewport",
            "rerun.archetypes.Scalars",
            "scalars",
            DataType::Float64,
        ),
        component(
            "/short",
            "rerun.archetypes.Points3D",
            "positions",
            DataType::Float32,
        ),
        component(
            "/short",
            "rerun.archetypes.Points3D",
            "colors",
            DataType::UInt32,
        ),
    ]);
    let mut harness = schema_harness(schema, [900.0, 450.0]);

    harness.get_by_label("short").click_accesskit();
    harness.run();

    let count = harness.get_by_label("2").rect();
    let heading = harness.get_by_label("2 components").rect();
    assert!(count.right() <= heading.left());
    assert!((count.center().y - harness.get_by_label("short").rect().center().y).abs() < 1.0);
    harness.snapshot("schema_page_tree_counts");

    scroll(&mut harness, count.center(), egui::vec2(-40.0, 0.0));
    assert!((harness.get_by_label("2").rect().right() - count.right()).abs() < 1.0);
}

/// A long data type shows only as many levels of nesting as fit its column, and the full type on hover.
/// Copying the cell copies the full type.
/// A long entity heading keeps the end of its path.
#[test]
fn schema_page_long_values() {
    const PATH: &str = "/world/robot/manipulator/calibration/cameras/external/stereo_rig/left_lens/estimated_camera_pose";
    let field = component(
        PATH,
        "rerun.archetypes.Calibration",
        "estimated_camera_calibration_parameters",
        DataType::Struct(
            vec![
                Field::new("translation_in_world_coordinates", DataType::Float64, false),
                Field::new("rotation_in_world_coordinates", DataType::Float64, false),
            ]
            .into(),
        ),
    );
    let datatype = field.data_type().to_string();
    let mut harness = schema_harness_with(
        re_ui::testing::new_harness(re_ui::testing::TestOptions::Gui, [1100.0, 450.0])
            .with_max_steps(10),
        schema_from_fields(vec![field]),
    );

    harness
        .get_by_label("estimated_camera_pose")
        .click_accesskit();
    harness.run();

    let name = harness
        .get_by_label("estimated_camera_calibration_parameters")
        .rect();
    let shortened = harness.get_by_label("List(Struct(…))").rect();
    assert!(shortened.left() >= name.right());
    harness.snapshot("schema_page_long_values");

    harness.hover_at(shortened.center());
    harness.run();
    harness.get_by_label(&datatype);
    harness.snapshot("schema_page_copy_button");

    harness.get_by_label("Copy").click();
    harness.step();
    assert!(
        harness
            .output()
            .platform_output
            .commands
            .contains(&egui::OutputCommand::CopyText(datatype.clone()))
    );

    let heading = harness
        .get(
            egui_kittest::kittest::By::new()
                .role(egui::accesskit::Role::Label)
                .predicate(|node| node.value().is_some_and(|value| value.starts_with('…'))),
        )
        .rect();
    assert!(heading.right() <= harness.get_by_label("1 component").rect().left());

    harness.hover_at(heading.center());
    harness.run();
    harness.get_by_label(PATH);
}

/// The schema page shows a searchable entity tree beside the selected entity's components.
/// Both viewer themes use the same layout.
#[test]
fn schema_page_screenshot() {
    let mut results = egui_kittest::SnapshotResults::new();

    for (theme, name) in [(egui::Theme::Dark, "dark"), (egui::Theme::Light, "light")] {
        let mut harness = schema_harness_with(
            re_ui::testing::new_harness(re_ui::testing::TestOptions::Gui, [1100.0, 600.0])
                .with_theme(theme),
            example_schema(),
        );

        harness.get_by_label("ext1").click_accesskit();
        harness.run();
        harness.snapshot(format!("schema_page_{name}"));

        results.extend_harness(&mut harness);
    }
}

/// Hovering a cell in the first column of the component table shows the archetype's full name.
#[test]
fn hover_first_column() {
    let mut harness = schema_harness(example_schema(), [1100.0, 600.0]);

    harness.get_by_label("ext2").click_accesskit();
    harness.run();

    let cell = harness.get_by_label("EncodedImage").rect();
    harness.hover_at(cell.center());
    harness.run();
    harness.get_by_label("rerun.archetypes.EncodedImage");
}

/// The schema page opens with no entity selected and prompts for one instead of a component table.
#[test]
fn schema_page_no_selection() {
    let harness = schema_harness(example_schema(), [1100.0, 600.0]);

    harness.get_by_label("Select an entity to see its components");
    assert!(harness.query_by_label("COMPONENT").is_none());
}

/// After clicking an entity, the arrow keys select the entity above or below it in the tree.
#[test]
fn arrow_keys_select_entities() {
    let mut harness = schema_harness(example_schema(), [1100.0, 600.0]);

    harness.get_by_label("ext1").click();
    harness.run();
    harness.get_by_label("/cameras/ext1");

    harness.key_press(egui::Key::ArrowDown);
    harness.run();
    harness.get_by_label("/cameras/ext2");

    harness.key_press(egui::Key::ArrowDown);
    harness.run();
    harness.get_by_label("/cameras/wrist");

    harness.key_press(egui::Key::ArrowUp);
    harness.run();
    harness.get_by_label("/cameras/ext2");
}

/// A nested entity row shows its full path on hover, while a row whose label already spells out the
/// whole path shows no tooltip.
#[test]
fn entity_row_tooltip() {
    let mut harness = schema_harness(example_schema(), [1100.0, 600.0]);

    harness.hover_at(harness.get_by_label("ext1").rect().center());
    harness.run();
    harness.get_by_label("/cameras/ext1");

    harness.hover_at(harness.get_by_label("cameras").rect().center());
    harness.run();
    assert!(harness.query_by_label("/cameras").is_none());

    harness.hover_at(harness.get_by_label("/ (root)").rect().center());
    harness.run();
    assert!(harness.query_by_label("/").is_none());
}

/// Display options toggle archetype-qualified names and an archetype column.
/// Selecting a parent entity shows its empty component state.
#[test]
fn schema_page_interaction() {
    let mut results = egui_kittest::SnapshotResults::new();
    let mut harness = schema_harness(example_schema(), [1100.0, 600.0]);

    harness.get_by_label("ext1").click_accesskit();
    harness.run();

    harness.get_by_label("Display").click();
    harness.run();
    harness.get_by_label("Full component names").click();
    harness.run();

    harness.get_by_label("Archetype").click();
    harness.run();
    assert!(harness.query_by_label("ARCHETYPE").is_none());
    harness.get_by_label("Archetype").click();
    harness.run();

    harness.get_by_label("Data type").click();
    harness.run();
    assert!(harness.query_by_label("DATA TYPE").is_none());
    harness.get_by_label("Data type").click();
    harness.run();

    harness.remove_cursor();
    harness.run();
    harness.snapshot("schema_page_display_menu");

    harness.key_press(egui::Key::Escape);
    harness.run();
    harness.get_by_label("EncodedImage:blob");
    harness.get_by_label("ARCHETYPE");
    harness.remove_cursor();
    harness.run();
    harness.snapshot("schema_page_display_options");

    harness.get_by_label("cameras").click();
    harness.run();
    harness.get_by_label("No components logged at this entity. It only groups children.");

    harness
        .get_by_role(egui::accesskit::Role::TextInput)
        .click();
    harness.run();
    harness
        .get_by_role(egui::accesskit::Role::TextInput)
        .type_text("WRIST");
    harness.run();

    harness.get_by_label("wrist").click();
    harness.run();
    harness.get_by_label("/cameras/wrist");
    harness.get_by_label("1 component");
    assert!(harness.query_by_label("ext1").is_none());
    harness.snapshot("schema_page_filtered");

    harness.get_by_label("Close").click();
    harness.run();
    assert_eq!(
        harness
            .get_by_role(egui::accesskit::Role::TextInput)
            .value()
            .as_deref(),
        Some("")
    );
    harness.get_by_label("ext1");

    harness
        .get_by_role(egui::accesskit::Role::TextInput)
        .click();
    harness.run();
    harness
        .get_by_role(egui::accesskit::Role::TextInput)
        .type_text("RAS/wr");
    harness.run();
    harness.remove_cursor();
    harness.run();
    harness.snapshot("schema_page_path_match");

    results.extend_harness(&mut harness);
}

/// The metadata column is hidden until it is turned on, and then counts the Arrow keys of each
/// column. Expanding one lists every key with its value.
#[test]
fn schema_page_metadata_column() {
    let mut harness = schema_harness(example_schema(), [1400.0, 600.0]);

    harness.get_by_label("ext1").click_accesskit();
    harness.run();
    assert!(harness.query_by_label("METADATA").is_none());

    show_metadata_column(&mut harness);
    harness.get_by_label("METADATA");
    harness.remove_cursor();
    harness.run();
    harness.snapshot("schema_page_metadata_column");

    assert!(harness.query_by_label("rerun:kind = data").is_none());
    harness
        .get_all_by_label("4 keys")
        .next()
        .expect("a component with no metadata beyond the four shared keys")
        .click();
    harness.run();
    let snippet = harness.get_by_label_contains("rerun:kind = data").rect();
    assert!(harness.query_by_label("Copy metadata").is_none());

    harness.hover_at(snippet.center());
    harness.run();
    harness.get_by_label("Copy metadata");
    harness.snapshot("schema_page_metadata_popup");

    harness.get_by_label("Copy metadata").click();
    harness.run();
    harness.get_by_label_contains("rerun:kind = data");
}

/// Every Arrow field metadata key is kept, including the ones the column descriptor already models.
#[test]
fn keeps_field_metadata() {
    let schema = example_schema();
    let metadata = &schema.entities[&EntityPath::from("/scene/points")].components[0].metadata;

    assert_eq!(metadata[re_sorbet::RERUN_KIND], "data");
    assert_eq!(
        metadata[re_sorbet::metadata::SORBET_ENTITY_PATH],
        "/scene/points"
    );
    assert_eq!(metadata["rerun:archetype"], "rerun.archetypes.Points3D");
    assert_eq!(metadata["rerun:component"], "Points3D:colors");
}
