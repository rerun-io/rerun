//! Blueprints shared by the tests.

use std::sync::Arc;

use arrow::array::ArrayRef;
use serde_json::Value;

use re_chunk::{Chunk, RowId};
use re_chunk_store::LatestAtQuery;
use re_entity_db::EntityDb;
use re_log_types::{EntityPath, StoreId, StoreKind, TimePoint, TimelineName};
use re_sdk_types::ComponentDescriptor;
use re_sdk_types::archetypes::Points3D;
use re_sdk_types::blueprint::archetypes::{
    ActiveVisualizers, ContainerBlueprint, EntityBehavior, EyeControls3D, PanelBlueprint, TimeAxis,
    TimePanelBlueprint, ViewBlueprint, ViewContents, ViewportBlueprint, VisualizerInstruction,
};
use re_sdk_types::blueprint::components::{
    ActiveTab, ContainerKind, Eye3DKind, IncludedContent, PanelState, PlayState, QueryExpression,
    RootContainer, ViewMaximized, VisualizerInstructionId,
};
use re_sdk_types::components::Color;
use re_sdk_types::datatypes::{TimeInt, TimeRange, TimeRangeBoundary, Uuid};

use crate::json_from_store;

pub fn empty_blueprint() -> EntityDb {
    EntityDb::new(StoreId::random(StoreKind::Blueprint, "test"))
}

/// A blueprint with one of every kind of pointer between blueprint entities, and of every kind of
/// entity below a view, laid out the way the viewer and the SDKs write them:
///
/// * A horizontal root container holding a 3D view and a tabs container of two more views.
/// * `root_container`, `contents`, `active_tab` and `maximized`, which point at other tiles.
/// * View properties (`ViewContents`, `EyeControls3D`, `TimeAxis`), component defaults, and an
///   entity override with its visualizer instruction.
/// * Top-level panels, a component logged without an archetype, and cleared and `Null`-typed
///   components, which are left out.
pub fn example_blueprint() -> EntityDb {
    let uuid = |n: u128| uuid::Uuid::from_u128(0x0000_0000_0000_4000_8000_0000_0000_0000 | n);
    let root = uuid(1);
    let tabs = uuid(2);
    let scene = uuid(3);
    let plot = uuid(4);
    let readme = uuid(5);
    let instruction = uuid(6);

    let content = |kind: &str, id: uuid::Uuid| {
        IncludedContent::from(&EntityPath::from(format!("{kind}/{id}").as_str()))
    };
    let scene_path = format!("view/{scene}");
    let readme_path = format!("view/{readme}");
    let points_visualizers = ViewContents::blueprint_base_visualizer_path_for_entity(
        scene,
        &EntityPath::from("/world/robot/points"),
    );

    let rows: Vec<(String, Box<dyn re_sdk_types::AsComponents>)> = vec![
        (
            "viewport".to_owned(),
            Box::new(
                ViewportBlueprint::new()
                    .with_root_container(RootContainer(Uuid::from(root)))
                    .with_maximized(ViewMaximized(Uuid::from(scene)))
                    .with_auto_layout(false)
                    .with_auto_views(false),
            ),
        ),
        (
            format!("container/{root}"),
            Box::new(
                ContainerBlueprint::new(ContainerKind::Horizontal)
                    .with_contents([content("view", scene), content("container", tabs)])
                    .with_col_shares([2.0, 1.0]),
            ),
        ),
        (
            format!("container/{tabs}"),
            Box::new(
                ContainerBlueprint::new(ContainerKind::Tabs)
                    .with_contents([content("view", plot), content("view", readme)])
                    .with_active_tab(ActiveTab::from(&EntityPath::from(readme_path.as_str()))),
            ),
        ),
        (
            scene_path.clone(),
            Box::new(
                ViewBlueprint::new("3D")
                    .with_display_name("Scene")
                    .with_space_origin("/world"),
            ),
        ),
        (
            format!("{scene_path}/ViewContents"),
            Box::new(ViewContents::new([
                QueryExpression::from("+ /world/**"),
                QueryExpression::from("- /world/debug/**"),
            ])),
        ),
        (
            format!("{scene_path}/EyeControls3D"),
            Box::new(
                EyeControls3D::update_fields()
                    .with_kind(Eye3DKind::Orbital)
                    .with_position([3.0, 3.0, 2.0]),
            ),
        ),
        (
            format!("{scene_path}/defaults"),
            Box::new(Points3D::update_fields().with_radii([0.02])),
        ),
        (
            points_visualizers.to_string(),
            Box::new(ActiveVisualizers::new([VisualizerInstructionId(
                Uuid::from(instruction),
            )])),
        ),
        (
            points_visualizers.to_string(),
            Box::new(EntityBehavior::update_fields().with_visible(true)),
        ),
        (
            format!("{points_visualizers}/{instruction}"),
            Box::new(VisualizerInstruction::new("Points3D")),
        ),
        (
            format!("{points_visualizers}/{instruction}"),
            Box::new(Points3D::update_fields().with_colors([Color::from_rgb(255, 0, 16)])),
        ),
        (
            format!("view/{plot}"),
            Box::new(ViewBlueprint::new("TimeSeries").with_space_origin("/joints")),
        ),
        (
            format!("view/{plot}/TimeAxis"),
            Box::new(TimeAxis::update_fields().with_view_range(TimeRange {
                start: TimeRangeBoundary::CursorRelative(TimeInt(-100)),
                end: TimeRangeBoundary::Infinite,
            })),
        ),
        (
            readme_path.clone(),
            Box::new(
                ViewBlueprint::new("TextDocument")
                    .with_display_name("README")
                    .with_space_origin("/description"),
            ),
        ),
        // Cleared, so left out entirely.
        (
            format!("{readme_path}/ViewContents"),
            Box::new(ViewContents::clear_fields()),
        ),
        (
            "time_panel".to_owned(),
            Box::new(
                TimePanelBlueprint::update_fields()
                    .with_fps(30.0)
                    .with_play_state(PlayState::Paused),
            ),
        ),
        (
            "blueprint_panel".to_owned(),
            Box::new(PanelBlueprint::update_fields().with_state(PanelState::Collapsed)),
        ),
    ];

    let mut db = empty_blueprint();
    for (entity_path, archetype) in rows {
        let chunk = Chunk::builder(entity_path.as_str())
            .with_archetype(RowId::new(), TimePoint::default(), archetype.as_ref())
            .build()
            .unwrap();
        db.add_chunk(&Arc::new(chunk)).unwrap();
    }

    // A component logged without an archetype, and a `Null`-typed one, as older blueprints
    // carry for indicators. The latter is left out.
    let note: ArrayRef = Arc::new(arrow::array::StringArray::from(vec!["hello"]));
    let indicator: ArrayRef = Arc::new(arrow::array::NullArray::new(1));
    let chunk = Chunk::builder(readme_path.as_str())
        .with_row(
            RowId::new(),
            TimePoint::default(),
            [
                (ComponentDescriptor::partial("my_note"), note),
                (ComponentDescriptor::partial("OldIndicator"), indicator),
            ],
        )
        .build()
        .unwrap();
    db.add_chunk(&Arc::new(chunk)).unwrap();

    db
}

pub fn read(blueprint: &EntityDb) -> Value {
    let query = LatestAtQuery::latest(TimelineName::from_static_str("blueprint"));
    json_from_store(
        blueprint.storage_engine().store(),
        &query,
        re_sdk_types::reflection::reflection(),
    )
}
