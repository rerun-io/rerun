use std::sync::Arc;

use ahash::HashMap;
use re_chunk::{ComponentIdentifier, RowId};
use re_chunk_store::{ChunkTrackingMode, LatestAtQuery};
use re_entity_db::EntityDb;
use re_log_types::{EntityPath, EntityPathHash, ResolvedEntityPathFilter, StoreId, Timeline};
use re_sdk_types::ViewClassIdentifier;
use re_sdk_types::blueprint::archetypes as blueprint_archetypes;
use re_viewer_context::{
    ActiveStoreContext, AppCache, AppCaches, AppOptions, Cache, DataQueryResult, DataResultNode,
    IndicatedEntities, PerVisualizerType, QueryRange, ViewClassRegistry, ViewId, ViewStates,
    VisualizableEntities,
};

use crate::ViewBlueprint;

/// Returns the [`DataQueryResult`] of each of the given views.
///
/// Results are kept in [`AppCaches`] across frames.
/// Only views whose inputs changed since their result was built get rebuilt, in parallel.
/// A view without a state in `view_states` gets no result.
///
/// Recordings whose schema is that of an RRD manifest with the same schema hash share results.
/// This holds for the segments of a dataset that are shown with the same blueprint.
///
/// A result shared between recordings is built with the view state of one of them.
#[expect(clippy::too_many_arguments)]
pub fn query_results_for_views<'a>(
    store_context: &ActiveStoreContext<'_>,
    app_caches: &AppCaches,
    views: impl IntoIterator<Item = &'a ViewBlueprint>,
    view_class_registry: &ViewClassRegistry,
    blueprint_query: &LatestAtQuery,
    view_states: &ViewStates,
    visualizable_entities_per_visualizer: &PerVisualizerType<&VisualizableEntities>,
    indicated_entities_per_visualizer: &PerVisualizerType<&IndicatedEntities>,
    app_options: &AppOptions,
) -> HashMap<ViewId, Arc<DataQueryResult>> {
    re_tracing::profile_function!();

    use rayon::iter::{IntoParallelIterator as _, ParallelIterator as _};

    let recording = store_context.recording;
    let blueprint = store_context.blueprint;
    let active_timeline = store_context.time_ctrl.timeline();

    let recording_schema = RecordingSchema::new(recording);

    let mut query_results = HashMap::default();
    let mut views_to_rebuild = Vec::new();

    {
        re_tracing::profile_scope!("check_inputs");

        let views_with_inputs: Vec<_> = views
            .into_iter()
            .collect::<Vec<_>>()
            .into_par_iter()
            .filter_map(|view| {
                let Some(view_state) = view_states.get(recording.store_id(), view.id) else {
                    re_log::debug_warn_once!("Missing view state for view {:?}", view.id);
                    return None;
                };

                let query_range = view.query_range(
                    blueprint,
                    blueprint_query,
                    active_timeline,
                    view_class_registry,
                    view_state,
                );

                Some((
                    view,
                    query_range,
                    OverridesAndDefaults::new(blueprint, blueprint_query, view.id),
                ))
            })
            .collect();

        app_caches.memoizer(|cache: &mut DataQueryResultCache| {
            for (view, query_range, overrides_and_defaults) in views_with_inputs {
                let key = EntryKey {
                    blueprint_id: blueprint.store_id().clone(),
                    recording_schema: recording_schema.clone(),
                    view_id: view.id,
                };
                if let Some(entry) = cache.entries.get_mut(&key)
                    && entry.is_up_to_date(
                        active_timeline,
                        view,
                        &query_range,
                        &overrides_and_defaults,
                        app_options,
                    )
                {
                    entry.used_this_frame = true;
                    query_results.insert(view.id, entry.result.clone());
                } else {
                    views_to_rebuild.push((key, view, query_range, overrides_and_defaults));
                }
            }
        });
    }

    if views_to_rebuild.is_empty() {
        return query_results;
    }

    let rebuilt: Vec<_> = {
        re_tracing::profile_scope!(
            "rebuild_query_results",
            format!("{} views", views_to_rebuild.len()).as_str()
        );

        views_to_rebuild
            .into_par_iter()
            .map(|(key, view, query_range, overrides_and_defaults)| {
                re_tracing::profile_scope!(
                    "view",
                    view.display_name_or_default().to_string().as_str()
                );

                let result = view.contents.build_data_result_tree(
                    store_context,
                    active_timeline,
                    view_class_registry,
                    blueprint_query,
                    &query_range,
                    visualizable_entities_per_visualizer,
                    indicated_entities_per_visualizer,
                    app_options,
                );

                (
                    key,
                    Entry {
                        active_timeline: active_timeline.copied(),
                        class_identifier: view.class_identifier(),
                        entity_path_filter: view.contents.entity_path_filter().clone(),
                        query_range,
                        overrides_and_defaults,
                        app_options: app_options.clone(),
                        result: Arc::new(result),
                        used_this_frame: true,
                    },
                )
            })
            .collect()
    };

    app_caches.memoizer(|cache: &mut DataQueryResultCache| {
        for (key, entry) in rebuilt {
            query_results.insert(key.view_id, entry.result.clone());
            cache.entries.insert(key, entry);
        }
    });

    query_results
}

/// The schema of the recording.
///
/// The entity tree and the visualizable and indicated entities only depend on the schema.
#[derive(Clone, PartialEq, Eq, Hash)]
enum RecordingSchema {
    /// The schema hash of the recording's RRD manifest.
    ///
    /// Only used while every column and entity of the recording came from the manifest, and the
    /// manifest's schema covers all of its chunks.
    RrdManifest([u8; 32]),

    /// The schema generation of the store, which also changes when the store is replaced by a
    /// new one with the same id.
    Store { store_id: StoreId, generation: u64 },
}

impl RecordingSchema {
    fn new(recording: &EntityDb) -> Self {
        let engine = recording.storage_engine();
        let schema = engine.store().schema();

        if schema.is_from_rrd_manifests_only()
            && let Some(manifest) = recording.rrd_manifest_index().manifest()
            && manifest.schema_covers_all_chunks()
        {
            Self::RrdManifest(*manifest.sorbet_schema_sha256())
        } else {
            Self::Store {
                store_id: recording.store_id().clone(),
                generation: schema.generation(),
            }
        }
    }
}

/// The latest rows of a view's overrides subtree and component defaults, at the current blueprint
/// query.
///
/// The overrides subtree holds the per-entity overrides and visualizer instructions.
///
/// TODO(#8233): Any change to an override or default rebuilds the view's whole [`DataQueryResult`],
/// e.g. every frame while the color picker edits an override.
/// Only the visualizer instructions and the entity tree should need a rebuild, with overrides and
/// defaults resolved outside of the [`DataQueryResult`].
#[derive(Default, PartialEq, Eq)]
struct OverridesAndDefaults {
    /// Every entity in the view's overrides subtree, whether or not it has data at the query time.
    override_entities: Vec<EntityPathHash>,

    /// The latest row of each component on each entity in the view's overrides subtree.
    ///
    /// These are the per-entity component overrides and visualizer instructions.
    override_rows: Vec<LatestComponentRow>,

    /// The latest row of each component on the view's defaults entity.
    default_rows: Vec<LatestComponentRow>,
}

/// The row of the latest value of a component on a blueprint entity, at the blueprint query.
#[derive(PartialEq, Eq)]
struct LatestComponentRow {
    entity: EntityPathHash,
    component: ComponentIdentifier,
    row_id: RowId,
}

impl OverridesAndDefaults {
    fn new(blueprint: &EntityDb, blueprint_query: &LatestAtQuery, view_id: ViewId) -> Self {
        let engine = blueprint.storage_engine();
        let entity_tree = engine.store().entity_tree();

        let mut inputs = Self::default();

        let add_rows = |rows: &mut Vec<LatestComponentRow>, entity_path: &EntityPath| {
            let Some(components) = engine.schema().all_components_for_entity(entity_path) else {
                return;
            };
            let results = engine.cache().latest_at(
                ChunkTrackingMode::Report,
                blueprint_query,
                entity_path,
                components.iter().copied(),
            );
            for &component in components {
                if let Some(row_id) = results.component_row_id(component) {
                    rows.push(LatestComponentRow {
                        entity: entity_path.hash(),
                        component,
                        row_id,
                    });
                }
            }
        };

        let overrides_path =
            blueprint_archetypes::ViewContents::blueprint_overrides_path_for_view(view_id.uuid());
        if let Some(overrides) = entity_tree.subtree(&overrides_path) {
            overrides.visit_children_recursively(|entity_path| {
                inputs.override_entities.push(entity_path.hash());
                add_rows(&mut inputs.override_rows, entity_path);
            });
        }

        add_rows(
            &mut inputs.default_rows,
            &ViewBlueprint::defaults_path(view_id),
        );

        inputs
    }
}

/// Recordings with the same schema share entries, so switching between the segments of a dataset
/// reuses the results of the previous one.
/// Recordings with different schemas have separate entries, also when they are shown with the
/// same blueprint in the same frame.
#[derive(Clone, PartialEq, Eq, Hash)]
struct EntryKey {
    blueprint_id: StoreId,
    recording_schema: RecordingSchema,
    view_id: ViewId,
}

/// A view's [`DataQueryResult`] and the inputs it was built from.
struct Entry {
    active_timeline: Option<Timeline>,
    class_identifier: ViewClassIdentifier,
    entity_path_filter: ResolvedEntityPathFilter,
    query_range: QueryRange,
    overrides_and_defaults: OverridesAndDefaults,
    app_options: AppOptions,
    result: Arc<DataQueryResult>,
    used_this_frame: bool,
}

impl Entry {
    fn is_up_to_date(
        &self,
        active_timeline: Option<&Timeline>,
        view: &ViewBlueprint,
        query_range: &QueryRange,
        overrides_and_defaults: &OverridesAndDefaults,
        app_options: &AppOptions,
    ) -> bool {
        self.active_timeline.as_ref() == active_timeline
            && self.class_identifier == view.class_identifier()
            && &self.entity_path_filter == view.contents.entity_path_filter()
            && &self.query_range == query_range
            && &self.overrides_and_defaults == overrides_and_defaults
            && &self.app_options == app_options
    }
}

/// The [`DataQueryResult`] of each view, across frames.
///
/// Entries that were neither looked up nor rebuilt since the last frame get dropped.
#[derive(Default)]
struct DataQueryResultCache {
    entries: HashMap<EntryKey, Entry>,
}

impl Cache for DataQueryResultCache {
    fn name(&self) -> &'static str {
        "DataQueryResultCache"
    }

    fn begin_frame(&mut self) {
        re_tracing::profile_function!();

        self.entries.retain(|_, entry| entry.used_this_frame);

        #[expect(clippy::iter_over_hash_type)] // Resetting a flag is order-independent.
        for entry in self.entries.values_mut() {
            entry.used_this_frame = false;
        }
    }

    fn purge_memory(&mut self) {
        self.entries.retain(|_, entry| entry.used_this_frame);
    }
}

impl AppCache for DataQueryResultCache {}

impl re_byte_size::MemUsageTreeCapture for DataQueryResultCache {
    fn capture_mem_usage_tree(&self) -> re_byte_size::MemUsageTree {
        let num_nodes: usize = self
            .entries
            .values()
            .map(|entry| entry.result.tree.data_results.len())
            .sum();
        re_byte_size::MemUsageTree::Bytes((num_nodes * size_of::<DataResultNode>()) as u64)
    }
}

#[cfg(test)]
mod tests {
    use re_chunk::Chunk;
    use re_log_types::example_components::{MyPoint, MyPoints};
    use re_log_types::{StoreKind, TimeInt, TimePoint};
    use re_types_core::reflection::Reflection;
    use re_viewer_context::{
        FallbackProviderRegistry, IdentifiedViewSystem as _, StoreCache, TimeControl,
        ViewClass as _, VisualizableReason, blueprint_timeline,
    };

    use super::*;
    use crate::ViewContents;
    use crate::test_view_class::{TestViewClass, TestVisualizer};

    fn points_chunk(entity_path: &EntityPath, timepoint: TimePoint) -> Chunk {
        Chunk::builder(entity_path.clone())
            .with_component_batch(
                RowId::new(),
                timepoint,
                (
                    MyPoints::descriptor_points(),
                    &[MyPoint::new(1.0, 2.0)] as _,
                ),
            )
            .build()
            .unwrap()
    }

    fn add_points(db: &mut EntityDb, entity_path: &EntityPath, timepoint: TimePoint) {
        db.add_chunk(&Arc::new(points_chunk(entity_path, timepoint)))
            .unwrap();
    }

    fn frame_timepoint(frame: i64) -> TimePoint {
        TimePoint::from([(
            Timeline::new_sequence("frame"),
            TimeInt::new_temporal(frame),
        )])
    }

    /// A recording that only has an RRD manifest with points on each of the given entities.
    fn recording_from_manifest(entities: &[&str]) -> EntityDb {
        let store_id = StoreId::random(StoreKind::Recording, "test_app");
        let chunks: Vec<_> = entities
            .iter()
            .map(|path| points_chunk(&EntityPath::from(*path), frame_timepoint(0)))
            .collect();
        let manifest = re_chunk_index::RrdManifest::build_in_memory_from_chunks(
            store_id.clone(),
            chunks.iter(),
        )
        .unwrap();

        let mut recording = EntityDb::new(store_id);
        recording.add_rrd_manifest_message(manifest);
        recording
    }

    /// A recording with points on each of the given entities.
    fn recording_with(entities: &[&str]) -> EntityDb {
        let mut recording = EntityDb::new(StoreId::random(StoreKind::Recording, "test_app"));
        for path in entities {
            add_points(
                &mut recording,
                &EntityPath::from(*path),
                TimePoint::default(),
            );
        }
        recording
    }

    fn latest() -> LatestAtQuery {
        LatestAtQuery::latest(blueprint_timeline())
    }

    fn blueprint_query_at(time: i64) -> LatestAtQuery {
        LatestAtQuery::new(blueprint_timeline(), time)
    }

    /// An empty blueprint with two views that show everything.
    struct Setup {
        view_class_registry: ViewClassRegistry,
        blueprint: EntityDb,
        app_caches: AppCaches,
        views: [ViewBlueprint; 2],
        view_states: ViewStates,
        visualizable_entities: PerVisualizerType<VisualizableEntities>,
        app_options: AppOptions,
    }

    impl Setup {
        fn new() -> Self {
            let mut view_class_registry = ViewClassRegistry::default();
            view_class_registry
                .add_class::<TestViewClass>(
                    &Reflection::default(),
                    &AppOptions::test(),
                    &mut FallbackProviderRegistry::default(),
                )
                .unwrap();

            let mut visualizable_entities = PerVisualizerType::<VisualizableEntities>::default();
            visualizable_entities.0.insert(
                TestVisualizer::identifier(),
                VisualizableEntities(
                    ["first", "second"]
                        .into_iter()
                        .map(|path| (EntityPath::from(path), VisualizableReason::Always))
                        .collect(),
                ),
            );

            Self {
                view_class_registry,
                blueprint: EntityDb::new(StoreId::random(StoreKind::Blueprint, "test_app")),
                app_caches: AppCaches::default(),
                views: [
                    ViewBlueprint::new_with_root_wildcard(TestViewClass::identifier()),
                    ViewBlueprint::new_with_root_wildcard(TestViewClass::identifier()),
                ],
                view_states: ViewStates::default(),
                visualizable_entities,
                app_options: AppOptions::test(),
            }
        }

        fn run(
            &mut self,
            recording: &EntityDb,
            blueprint_query: &LatestAtQuery,
        ) -> [Arc<DataQueryResult>; 2] {
            for view in &self.views {
                self.view_states
                    .ensure_state_exists(recording.store_id(), view.id, &TestViewClass);
            }

            let caches = StoreCache::new(&self.view_class_registry, recording);
            let time_ctrl = TimeControl::default();
            let store_context = ActiveStoreContext {
                blueprint: &self.blueprint,
                default_blueprint: None,
                recording,
                caches: &caches,
                time_ctrl: &time_ctrl,
                should_enable_heuristics: false,
            };
            let mut results = query_results_for_views(
                &store_context,
                &self.app_caches,
                &self.views,
                &self.view_class_registry,
                blueprint_query,
                &self.view_states,
                &self.visualizable_entities.as_ref(),
                &PerVisualizerType::default(),
                &self.app_options,
            );
            self.views.each_ref().map(|view| {
                results
                    .remove(&view.id)
                    .expect("the view has a state, so it gets a result")
            })
        }

        /// Writes a blueprint row that no view's query reads.
        fn write_unrelated(&mut self, time: i64) {
            let path = self.views[0].id.as_entity_path().join(&"unrelated".into());
            let timepoint =
                TimePoint::from_iter([(Timeline::new_sequence(blueprint_timeline()), time)]);
            add_points(&mut self.blueprint, &path, timepoint);
        }

        /// Hides the recording's entity in the first view.
        fn write_override(&mut self, time: i64) {
            let path =
                ViewContents::base_override_path_for_entity(self.views[0].id, &"first".into());
            let chunk = Chunk::builder(path)
                .with_archetype(
                    RowId::new(),
                    TimePoint::from_iter([(Timeline::new_sequence(blueprint_timeline()), time)]),
                    &blueprint_archetypes::EntityBehavior::new().with_visible(false),
                )
                .build()
                .unwrap();
            self.blueprint.add_chunk(&Arc::new(chunk)).unwrap();
        }
    }

    fn has_entity(result: &DataQueryResult, entity_path: &str) -> bool {
        result
            .tree
            .lookup_result_by_path(EntityPath::from(entity_path).hash())
            .is_some()
    }

    fn is_visible(result: &DataQueryResult) -> bool {
        result
            .tree
            .lookup_result_by_path(EntityPath::from("first").hash())
            .expect("the entity is in the view")
            .visible
    }

    fn rebuilt(before: &[Arc<DataQueryResult>; 2], after: &[Arc<DataQueryResult>; 2]) -> [bool; 2] {
        [0, 1].map(|i| !Arc::ptr_eq(&before[i], &after[i]))
    }

    /// A view's result is reused while nothing changes, also when new rows arrive for an entity
    /// and component the recording already has. A new entity rebuilds every result.
    /// Results that go unused for a whole frame are dropped from the cache.
    #[test]
    fn rebuilds_only_when_inputs_change() {
        let mut setup = Setup::new();
        let mut recording = recording_with(&["first"]);

        let first = setup.run(&recording, &latest());
        assert_eq!(
            rebuilt(&first, &setup.run(&recording, &latest())),
            [false, false]
        );

        add_points(&mut recording, &"first".into(), frame_timepoint(1));
        assert_eq!(
            rebuilt(&first, &setup.run(&recording, &latest())),
            [false, false]
        );

        add_points(&mut recording, &"second".into(), TimePoint::default());
        let second = setup.run(&recording, &latest());
        assert_eq!(rebuilt(&first, &second), [true, true]);
        assert!(has_entity(&second[0], "second"));

        setup.app_caches.begin_frame();
        setup.app_caches.begin_frame();
        assert_eq!(
            rebuilt(&second, &setup.run(&recording, &latest())),
            [true, true]
        );
    }

    /// Changing any of the app options rebuilds every result.
    #[test]
    fn app_options_change_rebuilds() {
        let mut setup = Setup::new();
        let recording = recording_with(&["first"]);
        let before = setup.run(&recording, &latest());

        setup.app_options.show_metrics = !setup.app_options.show_metrics;
        assert_eq!(
            rebuilt(&before, &setup.run(&recording, &latest())),
            [true, true]
        );
    }

    /// Replacing the recording with a new one under the same id, as clearing its data does,
    /// rebuilds every result, also when the new recording gets as many schema changes.
    #[test]
    fn replaced_recording_rebuilds() {
        let mut setup = Setup::new();
        let recording = recording_with(&["first"]);
        let before = setup.run(&recording, &latest());

        let mut replaced = EntityDb::new(recording.store_id().clone());
        add_points(&mut replaced, &"second".into(), TimePoint::default());
        let after = setup.run(&replaced, &latest());

        assert_eq!(rebuilt(&before, &after), [true, true]);
        assert!(!has_entity(&after[0], "first"));
    }

    /// Recordings whose RRD manifests have the same schema share their results.
    /// A recording with a different schema gets results of its own, and shown in the same frame
    /// with the same blueprint, both keep their results.
    #[test]
    fn recordings_with_same_manifest_schema_share_results() {
        let mut setup = Setup::new();
        let first = recording_from_manifest(&["first"]);
        let second = recording_from_manifest(&["first"]);
        let other = recording_from_manifest(&["first", "second"]);

        let first_results = setup.run(&first, &latest());
        assert_eq!(
            rebuilt(&first_results, &setup.run(&second, &latest())),
            [false, false]
        );
        let other_results = setup.run(&other, &latest());
        assert_eq!(rebuilt(&first_results, &other_results), [true, true]);

        setup.app_caches.begin_frame();
        assert_eq!(
            rebuilt(&first_results, &setup.run(&first, &latest())),
            [false, false]
        );
        assert_eq!(
            rebuilt(&other_results, &setup.run(&other, &latest())),
            [false, false]
        );
    }

    /// Loading a chunk the RRD manifest lists keeps sharing results.
    /// A chunk with an entity the manifest lacks gives the recording results of its own.
    #[test]
    fn data_outside_manifest_stops_sharing() {
        let mut setup = Setup::new();
        let first = recording_from_manifest(&["first"]);
        let mut second = recording_from_manifest(&["first"]);

        let first_results = setup.run(&first, &latest());

        add_points(&mut second, &"first".into(), frame_timepoint(0));
        assert_eq!(
            rebuilt(&first_results, &setup.run(&second, &latest())),
            [false, false]
        );

        add_points(&mut second, &"second".into(), frame_timepoint(0));
        let second_results = setup.run(&second, &latest());
        assert_eq!(rebuilt(&first_results, &second_results), [true, true]);
        assert!(has_entity(&second_results[0], "second"));
    }

    /// A blueprint write that no view's query reads keeps every result, also when the blueprint
    /// query moves back to before the write.
    #[test]
    fn unrelated_blueprint_write_rebuilds_nothing() {
        let mut setup = Setup::new();
        let recording = recording_with(&["first"]);
        setup.write_override(1);

        let before = setup.run(&recording, &latest());
        setup.write_unrelated(2);
        let after = setup.run(&recording, &latest());
        assert_eq!(rebuilt(&before, &after), [false, false]);

        let undone = setup.run(&recording, &blueprint_query_at(1));
        assert_eq!(rebuilt(&after, &undone), [false, false]);
        assert!(!is_visible(&undone[0]));
    }

    /// Writing an override in one view rebuilds only that view.
    /// Moving the blueprint query back to before the override rebuilds that view again.
    #[test]
    fn override_write_rebuilds_only_that_view() {
        let mut setup = Setup::new();
        let recording = recording_with(&["first"]);

        let before = setup.run(&recording, &latest());
        assert!(is_visible(&before[0]));

        setup.write_override(1);
        let after = setup.run(&recording, &latest());
        assert_eq!(rebuilt(&before, &after), [true, false]);
        assert!(!is_visible(&after[0]));

        let undone = setup.run(&recording, &blueprint_query_at(0));
        assert_eq!(rebuilt(&after, &undone), [true, false]);
        assert!(is_visible(&undone[0]));
    }
}
