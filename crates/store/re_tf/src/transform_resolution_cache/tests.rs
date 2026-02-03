use std::sync::{Arc, OnceLock};

use glam::DAffine3;
use re_chunk_store::{
    Chunk, ChunkStore, ChunkStoreEvent, ChunkStoreSubscriberHandle, GarbageCollectionOptions,
    LatestAtQuery, PerStoreChunkSubscriber,
};
use re_entity_db::EntityDb;
use re_log_types::{
    EntityPath, StoreId, StoreInfo, TimeInt, TimePoint, Timeline, TimelineName,
    example_components::{MyPoint, MyPoints},
};
use re_sdk_types::{
    ChunkId,
    archetypes::{self, InstancePoses3D, Pinhole, Transform3D},
    components::{self, PinholeProjection},
};

use crate::TransformFrameIdHash;
use crate::convert;

use super::{ParentFromChildTransform, ResolvedPinholeProjection, TransformResolutionCache};

#[derive(Debug, Clone, Copy)]
enum StaticTestFlavor {
    /// First log a static chunk and then a regular chunk.
    StaticThenRegular { update_inbetween: bool },

    /// First log a regular chunk and then a static chunk.
    RegularThenStatic { update_inbetween: bool },

    /// Test case where we first log a static chunk and regular chunk and then later swap out the static chunk.
    /// This tests that we're able to invalidate the cache on static changes after the fact.
    PriorStaticThenRegularThenStatic { update_inbetween: bool },
}

const ALL_STATIC_TEST_FLAVOURS: [StaticTestFlavor; 6] = [
    StaticTestFlavor::StaticThenRegular {
        update_inbetween: true,
    },
    StaticTestFlavor::RegularThenStatic {
        update_inbetween: true,
    },
    StaticTestFlavor::PriorStaticThenRegularThenStatic {
        update_inbetween: true,
    },
    StaticTestFlavor::StaticThenRegular {
        update_inbetween: false,
    },
    StaticTestFlavor::RegularThenStatic {
        update_inbetween: false,
    },
    StaticTestFlavor::PriorStaticThenRegularThenStatic {
        update_inbetween: false,
    },
];

#[derive(Default)]
pub struct TestStoreSubscriber {
    unprocessed_events: Vec<ChunkStoreEvent>,
}

impl TestStoreSubscriber {
    /// Accesses the global store subscriber.
    ///
    /// Lazily registers the subscriber if it hasn't been registered yet.
    pub fn subscription_handle() -> ChunkStoreSubscriberHandle {
        static SUBSCRIPTION: OnceLock<ChunkStoreSubscriberHandle> = OnceLock::new();
        *SUBSCRIPTION.get_or_init(ChunkStore::register_per_store_subscriber::<Self>)
    }

    /// Retrieves all transform events that have not been processed yet since the last call to this function.
    pub fn take_transform_events(store_id: &StoreId) -> Vec<ChunkStoreEvent> {
        ChunkStore::with_per_store_subscriber_mut(
            Self::subscription_handle(),
            store_id,
            |subscriber: &mut Self| std::mem::take(&mut subscriber.unprocessed_events),
        )
        .unwrap_or_default()
    }
}

impl PerStoreChunkSubscriber for TestStoreSubscriber {
    fn name() -> String {
        "TestStoreSubscriber".to_owned()
    }

    fn on_events<'a>(&mut self, events: impl Iterator<Item = &'a ChunkStoreEvent>) {
        self.unprocessed_events.extend(events.cloned());
    }
}

/// Test helper that applies store subscriber events to the cache.
///
/// This also initializes any new timelines from the events.
fn apply_store_subscriber_events(cache: &mut TransformResolutionCache, entity_db: &EntityDb) {
    let events = TestStoreSubscriber::take_transform_events(entity_db.store_id());

    // Initialize any new timelines from the events.
    for event in &events {
        if let Some(chunk) = event.delta_chunk() {
            for timeline in chunk.timelines().keys() {
                if !cache.cached_timelines().any(|t| t == *timeline) {
                    cache.ensure_timeline_is_initialized(
                        entity_db.storage_engine().store(),
                        *timeline,
                    );
                }
            }
        }
    }

    cache.process_store_events(events.iter());
}

fn static_test_setup_store(
    cache: &mut TransformResolutionCache,
    prior_static_chunk: Chunk,
    final_static_chunk: Chunk,
    regular_chunk: Chunk,
    flavor: StaticTestFlavor,
) -> Result<EntityDb, Box<dyn std::error::Error>> {
    // Print the flavor to its shown on test failure.
    println!("{flavor:?}");

    let mut entity_db = new_entity_db_with_subscriber_registered();

    match flavor {
        StaticTestFlavor::StaticThenRegular { update_inbetween } => {
            entity_db.add_chunk(&Arc::new(final_static_chunk))?;
            if update_inbetween {
                apply_store_subscriber_events(cache, &entity_db);
            }
            entity_db.add_chunk(&Arc::new(regular_chunk))?;
        }

        StaticTestFlavor::RegularThenStatic { update_inbetween } => {
            entity_db.add_chunk(&Arc::new(regular_chunk))?;
            if update_inbetween {
                apply_store_subscriber_events(cache, &entity_db);
            }
            entity_db.add_chunk(&Arc::new(final_static_chunk))?;
        }

        StaticTestFlavor::PriorStaticThenRegularThenStatic { update_inbetween } => {
            entity_db.add_chunk(&Arc::new(prior_static_chunk))?;
            entity_db.add_chunk(&Arc::new(regular_chunk))?;
            if update_inbetween {
                apply_store_subscriber_events(cache, &entity_db);
            }
            entity_db.add_chunk(&Arc::new(final_static_chunk))?;
        }
    }

    Ok(entity_db)
}

fn new_entity_db_with_subscriber_registered() -> EntityDb {
    let entity_db = EntityDb::new(StoreInfo::testing().store_id);
    let _ = TestStoreSubscriber::subscription_handle();
    entity_db
}

#[test]
fn test_transforms_per_timeline_access() -> Result<(), Box<dyn std::error::Error>> {
    let mut entity_db = new_entity_db_with_subscriber_registered();
    let mut cache = TransformResolutionCache::default();

    // Log a few tree transforms at different times.
    let timeline = Timeline::new_sequence("t");
    let chunk0 = Chunk::builder(EntityPath::from("with_transform"))
        .with_archetype_auto_row(
            [(timeline, 1)],
            &Transform3D::from_translation([1.0, 2.0, 3.0]),
        )
        .build()?;
    let chunk1 = Chunk::builder(EntityPath::from("without_transform"))
        .with_archetype_auto_row(
            [(timeline, 1)],
            // Anything that doesn't have components the transform cache is interested in.
            &archetypes::Points3D::new([[1.0, 2.0, 3.0]]),
        )
        .build()?;
    entity_db.add_chunk(&Arc::new(chunk0))?;
    entity_db.add_chunk(&Arc::new(chunk1))?;

    apply_store_subscriber_events(&mut cache, &entity_db);
    let transforms_per_timeline = cache.transforms_for_timeline(*timeline.name());
    assert!(
        transforms_per_timeline
            .frame_transforms(TransformFrameIdHash::from_entity_path(&EntityPath::from(
                "without_transform"
            )))
            .is_none()
    );
    assert!(
        transforms_per_timeline
            .frame_transforms(TransformFrameIdHash::from_entity_path(&EntityPath::from(
                "rando"
            )))
            .is_none()
    );
    let transforms = transforms_per_timeline
        .frame_transforms(TransformFrameIdHash::from_entity_path(&EntityPath::from(
            "with_transform",
        )))
        .unwrap();
    #[cfg(debug_assertions)]
    assert_eq!(transforms.timeline, Some(*timeline.name()));
    assert_eq!(transforms.events.read().frame_transforms.len(), 1);
    assert_eq!(transforms.events.read().pinhole_projections.len(), 0);
    Ok(())
}

#[test]
fn test_static_tree_transforms() -> Result<(), Box<dyn std::error::Error>> {
    for flavor in &ALL_STATIC_TEST_FLAVOURS {
        // Log a few tree transforms at different times.
        let timeline = Timeline::new_sequence("t");
        let prior_static_chunk = Chunk::builder(EntityPath::from("my_entity"))
            .with_archetype_auto_row(
                TimePoint::default(),
                // Make sure only translation is logged (no null arrays for everything else).
                &Transform3D::from_translation([123.0, 234.0, 345.0]),
            )
            .build()?;
        let final_static_chunk = Chunk::builder(EntityPath::from("my_entity"))
            .with_archetype_auto_row(
                TimePoint::default(),
                // Make sure only translation is logged (no null arrays for everything else).
                &Transform3D::from_translation([1.0, 2.0, 3.0]),
            )
            .build()?;
        let regular_chunk = Chunk::builder(EntityPath::from("my_entity"))
            .with_archetype_auto_row(
                [(timeline, 1)],
                &Transform3D::from_scale([123.0, 234.0, 345.0]),
            )
            .build()?;

        let mut cache = TransformResolutionCache::default();
        let entity_db = static_test_setup_store(
            &mut cache,
            prior_static_chunk,
            final_static_chunk,
            regular_chunk,
            *flavor,
        )?;

        // Check that the transform cache has the expected transforms.
        apply_store_subscriber_events(&mut cache, &entity_db);

        let transforms_per_timeline = cache.transforms_for_timeline(*timeline.name());
        let transforms = transforms_per_timeline
            .frame_transforms(TransformFrameIdHash::from_entity_path(&EntityPath::from(
                "my_entity",
            )))
            .unwrap();

        assert_eq!(
            transforms.latest_at_transform(
                &entity_db,
                &LatestAtQuery::new(*timeline.name(), TimeInt::MIN)
            ),
            Some(ParentFromChildTransform {
                parent: TransformFrameIdHash::entity_path_hierarchy_root(),
                transform: DAffine3::from_translation(glam::dvec3(1.0, 2.0, 3.0)),
            })
        );
        assert_eq!(
            transforms.latest_at_transform(
                &entity_db,
                &LatestAtQuery::new(*timeline.name(), TimeInt::MIN)
            ),
            transforms.latest_at_transform(&entity_db, &LatestAtQuery::new(*timeline.name(), 0)),
        );
        assert_eq!(
            transforms.latest_at_transform(&entity_db, &LatestAtQuery::new(*timeline.name(), 1)),
            Some(ParentFromChildTransform {
                parent: TransformFrameIdHash::entity_path_hierarchy_root(),
                // Due to atomic-latest-at, the translation is no longer visible despite being on the static chunk.
                transform: DAffine3::from_scale(glam::dvec3(123.0, 234.0, 345.0)),
            })
        );

        // Timelines that the cache has never seen should still have the static transform.
        let transforms_per_timeline = cache.transforms_for_timeline(TimelineName::new("other"));
        let transforms = transforms_per_timeline
            .frame_transforms(TransformFrameIdHash::from_entity_path(&EntityPath::from(
                "my_entity",
            )))
            .unwrap();
        assert_eq!(
            transforms.latest_at_transform(
                &entity_db,
                &LatestAtQuery::new(TimelineName::new("other"), 123)
            ),
            Some(ParentFromChildTransform {
                parent: TransformFrameIdHash::entity_path_hierarchy_root(),
                transform: DAffine3::from_translation(glam::dvec3(1.0, 2.0, 3.0)),
            })
        );
    }

    Ok(())
}

#[test]
fn test_static_pose_transforms() -> Result<(), Box<dyn std::error::Error>> {
    for flavor in &ALL_STATIC_TEST_FLAVOURS {
        // Log a few tree transforms at different times.
        let timeline = Timeline::new_sequence("t");
        let prior_static_chunk = Chunk::builder(EntityPath::from("my_entity"))
            .with_archetype_auto_row(
                TimePoint::default(),
                &InstancePoses3D::new().with_translations([[321.0, 234.0, 345.0]]),
            )
            .build()?;
        let final_static_chunk = Chunk::builder(EntityPath::from("my_entity"))
            .with_archetype_auto_row(
                TimePoint::default(),
                &InstancePoses3D::new().with_translations([[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]]),
            )
            .build()?;
        let regular_chunk = Chunk::builder(EntityPath::from("my_entity"))
            .with_archetype_auto_row(
                [(timeline, 1)],
                // Add a splatted scale.
                &InstancePoses3D::new().with_scales([[10.0, 20.0, 30.0]]),
            )
            .build()?;

        let mut cache = TransformResolutionCache::default();
        let entity_db = static_test_setup_store(
            &mut cache,
            prior_static_chunk,
            final_static_chunk,
            regular_chunk,
            *flavor,
        )?;

        // Check that the transform cache has the expected transforms.
        apply_store_subscriber_events(&mut cache, &entity_db);

        let transforms_per_timeline = cache.transforms_for_timeline(*timeline.name());
        let transforms = transforms_per_timeline
            .pose_transforms(EntityPath::from("my_entity").hash())
            .unwrap();

        assert_eq!(
            transforms.latest_at_instance_poses(
                &entity_db,
                &LatestAtQuery::new(*timeline.name(), TimeInt::MIN)
            ),
            vec![
                DAffine3::from_translation(glam::dvec3(1.0, 2.0, 3.0)),
                DAffine3::from_translation(glam::dvec3(4.0, 5.0, 6.0)),
            ],
        );
        assert_eq!(
            transforms.latest_at_instance_poses(
                &entity_db,
                &LatestAtQuery::new(*timeline.name(), TimeInt::MIN)
            ),
            transforms
                .latest_at_instance_poses(&entity_db, &LatestAtQuery::new(*timeline.name(), 0)),
        );
        assert_eq!(
            transforms
                .latest_at_instance_poses(&entity_db, &LatestAtQuery::new(*timeline.name(), 1)),
            // Due to atomic-latest-at, the translation is no longer visible despite being on the static chunk.
            vec![DAffine3::from_scale(glam::dvec3(10.0, 20.0, 30.0)),]
        );

        // Timelines that the cache has never seen should still have the static poses.
        let transforms_per_timeline = cache.transforms_for_timeline(TimelineName::new("other"));
        let transforms = transforms_per_timeline
            .pose_transforms(EntityPath::from("my_entity").hash())
            .unwrap();
        assert_eq!(
            transforms.latest_at_instance_poses(
                &entity_db,
                &LatestAtQuery::new(TimelineName::new("other"), 123)
            ),
            vec![
                DAffine3::from_translation(glam::dvec3(1.0, 2.0, 3.0)),
                DAffine3::from_translation(glam::dvec3(4.0, 5.0, 6.0)),
            ]
        );
    }

    Ok(())
}

#[test]
fn test_static_pinhole_projection() -> Result<(), Box<dyn std::error::Error>> {
    for flavor in &ALL_STATIC_TEST_FLAVOURS {
        let image_from_camera_prior = PinholeProjection::from_focal_length_and_principal_point(
            [123.0, 123.0],
            [123.0, 123.0],
        );
        let image_from_camera_final =
            PinholeProjection::from_focal_length_and_principal_point([1.0, 2.0], [1.0, 2.0]);

        // Static pinhole, non-static view coordinates.
        let timeline = Timeline::new_sequence("t");
        let prior_static_chunk = Chunk::builder(EntityPath::from("my_entity"))
            .with_archetype_auto_row(
                TimePoint::default(),
                &Pinhole::new(image_from_camera_prior).with_resolution([1.0, 1.0]),
            )
            .build()?;
        let final_static_chunk = Chunk::builder(EntityPath::from("my_entity"))
            .with_archetype_auto_row(
                TimePoint::default(),
                &Pinhole::new(image_from_camera_final).with_resolution([2.0, 2.0]),
            )
            .build()?;
        let regular_chunk = Chunk::builder(EntityPath::from("my_entity"))
            .with_archetype_auto_row([(timeline, 1)], &archetypes::ViewCoordinates::BLU())
            .build()?;

        let mut cache = TransformResolutionCache::default();
        let entity_db = static_test_setup_store(
            &mut cache,
            prior_static_chunk,
            final_static_chunk,
            regular_chunk,
            *flavor,
        )?;

        // Check that the transform cache has the expected transforms.
        apply_store_subscriber_events(&mut cache, &entity_db);

        let transforms_per_timeline = cache.transforms_for_timeline(*timeline.name());
        let transforms = transforms_per_timeline
            .frame_transforms(TransformFrameIdHash::from_entity_path(&EntityPath::from(
                "my_entity",
            )))
            .unwrap();

        assert_eq!(
            transforms.latest_at_pinhole(
                &entity_db,
                &LatestAtQuery::new(*timeline.name(), TimeInt::MIN)
            ),
            Some(ResolvedPinholeProjection {
                parent: TransformFrameIdHash::entity_path_hierarchy_root(),
                image_from_camera: image_from_camera_final,
                resolution: Some([2.0, 2.0].into()),
                view_coordinates: archetypes::Pinhole::DEFAULT_CAMERA_XYZ,
            })
        );
        assert_eq!(
            transforms.latest_at_pinhole(
                &entity_db,
                &LatestAtQuery::new(*timeline.name(), TimeInt::MIN)
            ),
            transforms.latest_at_pinhole(&entity_db, &LatestAtQuery::new(*timeline.name(), 0))
        );
        assert_eq!(
            transforms.latest_at_pinhole(&entity_db, &LatestAtQuery::new(*timeline.name(), 1)),
            Some(ResolvedPinholeProjection {
                parent: TransformFrameIdHash::entity_path_hierarchy_root(),
                image_from_camera: image_from_camera_final,
                resolution: Some([2.0, 2.0].into()),
                view_coordinates: components::ViewCoordinates::BLU,
            })
        );

        // Timelines that the cache has never seen should still have the static pinhole.
        let transforms_per_timeline = cache.transforms_for_timeline(TimelineName::new("other"));
        let transforms = transforms_per_timeline
            .frame_transforms(TransformFrameIdHash::from_entity_path(&EntityPath::from(
                "my_entity",
            )))
            .unwrap();
        assert_eq!(
            transforms.latest_at_pinhole(
                &entity_db,
                &LatestAtQuery::new(TimelineName::new("other"), 123)
            ),
            Some(ResolvedPinholeProjection {
                parent: TransformFrameIdHash::entity_path_hierarchy_root(),
                image_from_camera: image_from_camera_final,
                resolution: Some([2.0, 2.0].into()),
                view_coordinates: archetypes::Pinhole::DEFAULT_CAMERA_XYZ,
            })
        );
    }

    Ok(())
}

#[test]
fn test_static_view_coordinates_projection() -> Result<(), Box<dyn std::error::Error>> {
    for flavor in &ALL_STATIC_TEST_FLAVOURS {
        let image_from_camera =
            PinholeProjection::from_focal_length_and_principal_point([1.0, 2.0], [1.0, 2.0]);

        // Static view coordinates, non-static pinhole.
        let timeline = Timeline::new_sequence("t");
        let prior_static_chunk = Chunk::builder(EntityPath::from("my_entity"))
            .with_archetype_auto_row(TimePoint::default(), &archetypes::ViewCoordinates::BRU())
            .build()?;
        let final_static_chunk = Chunk::builder(EntityPath::from("my_entity"))
            .with_archetype_auto_row(TimePoint::default(), &archetypes::ViewCoordinates::BLU())
            .build()?;
        let regular_chunk = Chunk::builder(EntityPath::from("my_entity"))
            .with_archetype_auto_row([(timeline, 1)], &Pinhole::new(image_from_camera))
            .build()?;

        let mut cache = TransformResolutionCache::default();
        let entity_db = static_test_setup_store(
            &mut cache,
            prior_static_chunk,
            final_static_chunk,
            regular_chunk,
            *flavor,
        )?;

        // Check that the transform cache has the expected transforms.
        apply_store_subscriber_events(&mut cache, &entity_db);
        let transforms_per_timeline = cache.transforms_for_timeline(*timeline.name());
        let transforms = transforms_per_timeline
            .frame_transforms(TransformFrameIdHash::from_entity_path(&EntityPath::from(
                "my_entity",
            )))
            .unwrap();

        // There's view coordinates, but that doesn't show up.
        assert_eq!(
            transforms.latest_at_pinhole(
                &entity_db,
                &LatestAtQuery::new(*timeline.name(), TimeInt::MIN)
            ),
            None
        );
        assert_eq!(
            transforms.latest_at_pinhole(
                &entity_db,
                &LatestAtQuery::new(*timeline.name(), TimeInt::MIN)
            ),
            transforms.latest_at_pinhole(&entity_db, &LatestAtQuery::new(*timeline.name(), 0)),
        );
        // Once we get a pinhole camera, the view coordinates should be there.
        assert_eq!(
            transforms.latest_at_pinhole(&entity_db, &LatestAtQuery::new(*timeline.name(), 1)),
            Some(ResolvedPinholeProjection {
                parent: TransformFrameIdHash::entity_path_hierarchy_root(),
                image_from_camera,
                resolution: None,
                view_coordinates: components::ViewCoordinates::BLU,
            })
        );
    }

    Ok(())
}

#[test]
fn test_tree_transforms() -> Result<(), Box<dyn std::error::Error>> {
    let mut entity_db = new_entity_db_with_subscriber_registered();
    let mut cache = TransformResolutionCache::default();

    // Log a few tree transforms at different times.
    let timeline = Timeline::new_sequence("t");
    let chunk = Chunk::builder(EntityPath::from("my_entity"))
        .with_archetype_auto_row(
            [(timeline, 1)],
            &Transform3D::from_translation([1.0, 2.0, 3.0]),
        )
        .with_archetype_auto_row([(timeline, 3)], &Transform3D::from_scale([1.0, 2.0, 3.0]))
        .with_archetype_auto_row(
            [(timeline, 4)],
            &Transform3D::from_rotation(glam::Quat::from_rotation_x(1.0)),
        )
        .with_archetype_auto_row([(timeline, 5)], &Transform3D::clear_fields())
        .build()?;
    entity_db.add_chunk(&Arc::new(chunk))?;

    // Check that the transform cache has the expected transforms.
    apply_store_subscriber_events(&mut cache, &entity_db);
    let timeline_name = *timeline.name();
    let transforms_per_timeline = cache.transforms_for_timeline(timeline_name);
    let transforms = transforms_per_timeline
        .frame_transforms(TransformFrameIdHash::from_entity_path(&EntityPath::from(
            "my_entity",
        )))
        .unwrap();

    for (t, expected) in [
        (0, None),
        (
            1,
            Some(DAffine3::from_translation(glam::dvec3(1.0, 2.0, 3.0))),
        ),
        (
            2,
            Some(DAffine3::from_translation(glam::dvec3(1.0, 2.0, 3.0))),
        ),
        (3, Some(DAffine3::from_scale(glam::dvec3(1.0, 2.0, 3.0)))),
        (
            4,
            // Note: We must use the same conversion path as the actual implementation:
            // glam::Quat (f32) -> Quaternion (f32) -> glam::DQuat (f64)
            // This involves casting f32 components to f64 and renormalizing, which produces
            // slightly different values than directly computing in f64.
            Some(DAffine3::from_quat(
                convert::quaternion_to_dquat(re_sdk_types::datatypes::Quaternion::from(
                    glam::Quat::from_rotation_x(1.0),
                ))
                .unwrap(),
            )),
        ),
        (5, Some(DAffine3::IDENTITY)), // Empty transform is treated as connected with identity.
        (123, Some(DAffine3::IDENTITY)), // Empty transform is treated as connected with identity.
    ] {
        assert_eq!(
            transforms.latest_at_transform(&entity_db, &LatestAtQuery::new(timeline_name, t)),
            expected.map(|transform| ParentFromChildTransform {
                parent: TransformFrameIdHash::entity_path_hierarchy_root(),
                transform,
            }),
            "at time {t}"
        );
    }

    Ok(())
}

#[test]
fn test_pose_transforms_instance_poses() -> Result<(), Box<dyn std::error::Error>> {
    let mut entity_db = new_entity_db_with_subscriber_registered();
    let mut cache = TransformResolutionCache::default();

    // Log a few tree transforms at different times.
    let timeline = Timeline::new_sequence("t");
    let chunk = Chunk::builder(EntityPath::from("my_entity"))
        .with_archetype_auto_row(
            [(timeline, 1)],
            &InstancePoses3D::new().with_translations([
                [1.0, 2.0, 3.0],
                [4.0, 5.0, 6.0],
                [7.0, 8.0, 9.0],
            ]),
        )
        .with_archetype_auto_row(
            [(timeline, 3)],
            // Less instances, and a splatted scale.
            &InstancePoses3D::new()
                .with_translations([[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]])
                .with_scales([[2.0, 3.0, 4.0]]),
        )
        .with_archetype_auto_row([(timeline, 4)], &InstancePoses3D::clear_fields())
        .build()?;
    entity_db.add_chunk(&Arc::new(chunk))?;

    // Check that the transform cache has the expected transforms.
    apply_store_subscriber_events(&mut cache, &entity_db);
    let timeline = *timeline.name();
    let transforms_per_timeline = cache.transforms_for_timeline(timeline);
    let transforms = transforms_per_timeline
        .pose_transforms(EntityPath::from("my_entity").hash())
        .unwrap();

    for (t, poses) in [
        (0, Vec::new()),
        (
            1,
            vec![
                DAffine3::from_translation(glam::dvec3(1.0, 2.0, 3.0)),
                DAffine3::from_translation(glam::dvec3(4.0, 5.0, 6.0)),
                DAffine3::from_translation(glam::dvec3(7.0, 8.0, 9.0)),
            ],
        ),
        (
            2,
            vec![
                DAffine3::from_translation(glam::dvec3(1.0, 2.0, 3.0)),
                DAffine3::from_translation(glam::dvec3(4.0, 5.0, 6.0)),
                DAffine3::from_translation(glam::dvec3(7.0, 8.0, 9.0)),
            ],
        ),
        (
            3,
            vec![
                DAffine3::from_scale_rotation_translation(
                    glam::dvec3(2.0, 3.0, 4.0),
                    glam::DQuat::IDENTITY,
                    glam::dvec3(1.0, 2.0, 3.0),
                ),
                DAffine3::from_scale_rotation_translation(
                    glam::dvec3(2.0, 3.0, 4.0),
                    glam::DQuat::IDENTITY,
                    glam::dvec3(4.0, 5.0, 6.0),
                ),
            ],
        ),
        (4, Vec::new()),
        (123, Vec::new()),
    ] {
        assert_eq!(
            transforms.latest_at_instance_poses(&entity_db, &LatestAtQuery::new(timeline, t)),
            poses,
            "Unexpected result at time {t}"
        );
    }

    Ok(())
}

#[test]
fn test_pinhole_projections() -> Result<(), Box<dyn std::error::Error>> {
    let mut entity_db = new_entity_db_with_subscriber_registered();
    let mut cache = TransformResolutionCache::default();

    let image_from_camera =
        PinholeProjection::from_focal_length_and_principal_point([1.0, 2.0], [1.0, 2.0]);

    // Log a few tree transforms at different times.
    let timeline = Timeline::new_sequence("t");
    let chunk = Chunk::builder(EntityPath::from("my_entity"))
        .with_archetype_auto_row([(timeline, 1)], &Pinhole::new(image_from_camera))
        .with_archetype_auto_row([(timeline, 3)], &archetypes::ViewCoordinates::BLU())
        // Clear out the pinhole projection (this should yield nothing then for the remaining view coordinates.)
        .with_archetype_auto_row([(timeline, 4)], &Pinhole::clear_fields())
        .build()?;
    entity_db.add_chunk(&Arc::new(chunk))?;

    // Check that the transform cache has the expected transforms.
    apply_store_subscriber_events(&mut cache, &entity_db);
    let timeline = *timeline.name();
    let transforms_per_timeline = cache.transforms_for_timeline(timeline);
    let transforms = transforms_per_timeline
        .frame_transforms(TransformFrameIdHash::from_entity_path(&EntityPath::from(
            "my_entity",
        )))
        .unwrap();

    for (t, pinhole_view_coordinates) in [
        (0, None),
        (1, Some(Pinhole::DEFAULT_CAMERA_XYZ)),
        (2, Some(Pinhole::DEFAULT_CAMERA_XYZ)),
        (3, Some(components::ViewCoordinates::BLU)),
        (4, None), // View coordinates alone doesn't give us a pinhole projection from the transform cache.
        (123, None),
    ] {
        assert_eq!(
            transforms.latest_at_pinhole(&entity_db, &LatestAtQuery::new(timeline, t)),
            pinhole_view_coordinates.map(|view_coordinates| ResolvedPinholeProjection {
                parent: TransformFrameIdHash::entity_path_hierarchy_root(),
                image_from_camera,
                resolution: None,
                view_coordinates,
            }),
            "Unexpected result at time {t}"
        );
    }

    Ok(())
}

#[test]
fn test_out_of_order_updates() -> Result<(), Box<dyn std::error::Error>> {
    let mut entity_db = new_entity_db_with_subscriber_registered();
    let mut cache = TransformResolutionCache::default();

    // Log a few tree transforms at different times.
    let timeline = Timeline::new_sequence("t");
    let chunk = Chunk::builder(EntityPath::from("my_entity"))
        .with_archetype_auto_row(
            [(timeline, 1)],
            &Transform3D::from_translation([1.0, 2.0, 3.0]),
        )
        .with_archetype_auto_row(
            [(timeline, 3)],
            // Note that this clears anything that could be inserted at time 2 due to atomic-query semantics.
            &Transform3D::from_translation([2.0, 3.0, 4.0]),
        )
        .build()?;
    entity_db.add_chunk(&Arc::new(chunk))?;

    // Check that the transform cache has the expected transforms.
    apply_store_subscriber_events(&mut cache, &entity_db);
    let timeline = *timeline.name();

    {
        let transforms_per_timeline = cache.transforms_for_timeline(timeline);
        let transforms = transforms_per_timeline
            .frame_transforms(TransformFrameIdHash::from_entity_path(&EntityPath::from(
                "my_entity",
            )))
            .unwrap();

        // Check that the transform cache has the expected transforms.
        for (t, transform) in [
            (1, DAffine3::from_translation(glam::dvec3(1.0, 2.0, 3.0))),
            (2, DAffine3::from_translation(glam::dvec3(1.0, 2.0, 3.0))),
            (3, DAffine3::from_translation(glam::dvec3(2.0, 3.0, 4.0))),
        ] {
            assert_eq!(
                transforms.latest_at_transform(&entity_db, &LatestAtQuery::new(timeline, t)),
                Some(ParentFromChildTransform {
                    parent: TransformFrameIdHash::entity_path_hierarchy_root(),
                    transform,
                }),
                "Unexpected result at time {t}",
            );
        }
    }

    // Add a transform between the two.
    let timeline = Timeline::new_sequence("t");
    let chunk = Chunk::builder(EntityPath::from("my_entity"))
        .with_archetype_auto_row(
            [(timeline, 2)],
            &Transform3D::from_scale([-1.0, -2.0, -3.0]),
        )
        .build()?;
    entity_db.add_chunk(&Arc::new(chunk))?;

    // Check that the transform cache has the expected changed transforms.
    apply_store_subscriber_events(&mut cache, &entity_db);
    let timeline = *timeline.name();
    let transforms_per_timeline = cache.transforms_for_timeline(timeline);
    let transforms = transforms_per_timeline
        .frame_transforms(TransformFrameIdHash::from_entity_path(&EntityPath::from(
            "my_entity",
        )))
        .unwrap();

    // Check that the transform cache has the expected transforms.
    for (t, transform) in [
        (1, DAffine3::from_translation(glam::dvec3(1.0, 2.0, 3.0))),
        (2, DAffine3::from_scale(glam::dvec3(-1.0, -2.0, -3.0))),
        (3, DAffine3::from_translation(glam::dvec3(2.0, 3.0, 4.0))),
    ] {
        assert_eq!(
            transforms.latest_at_transform(&entity_db, &LatestAtQuery::new(timeline, t)),
            Some(ParentFromChildTransform {
                parent: TransformFrameIdHash::entity_path_hierarchy_root(),
                transform,
            }),
            "Unexpected result at time {t}",
        );
    }

    Ok(())
}

#[test]
fn test_clear_non_recursive() -> Result<(), Box<dyn std::error::Error>> {
    for (clear_in_separate_chunk, first_clear_then_data) in
        [(false, false), (true, false), (true, true)]
    {
        println!("clear_in_separate_chunk: {clear_in_separate_chunk}");
        println!("first_clear_then_data: {first_clear_then_data}");

        let mut entity_db = new_entity_db_with_subscriber_registered();
        let mut cache = TransformResolutionCache::default();

        let timeline = Timeline::new_sequence("t");
        let timeline_name = *timeline.name();

        let path = EntityPath::from("ent");
        let data_chunk = Chunk::builder(path.clone())
            .with_archetype_auto_row(
                [(timeline, 1)],
                &Transform3D::from_translation([1.0, 2.0, 3.0]),
            )
            .with_archetype_auto_row(
                [(timeline, 3)],
                &Transform3D::from_translation([3.0, 4.0, 5.0]),
            )
            .build()?;
        let clear_chunk = Chunk::builder(path.clone())
            .with_archetype_auto_row([(timeline, 2)], &archetypes::Clear::new(false))
            .build()?;

        if clear_in_separate_chunk && !first_clear_then_data {
            entity_db.add_chunk(&Arc::new(data_chunk))?;

            // If we're putting the clear in a separate chunk, we can try warming the cache and see whether we get the right transforms.
            {
                apply_store_subscriber_events(&mut cache, &entity_db);
                let transforms_per_timeline = cache.transforms_for_timeline(timeline_name);
                let transforms = transforms_per_timeline
                    .frame_transforms(TransformFrameIdHash::from_entity_path(&path))
                    .unwrap();
                assert_eq!(
                    transforms
                        .latest_at_transform(&entity_db, &LatestAtQuery::new(timeline_name, 1)),
                    Some(ParentFromChildTransform {
                        parent: TransformFrameIdHash::entity_path_hierarchy_root(),
                        transform: DAffine3::from_translation(glam::dvec3(1.0, 2.0, 3.0)),
                    })
                );
                assert_eq!(
                    transforms
                        .latest_at_transform(&entity_db, &LatestAtQuery::new(timeline_name, 3)),
                    Some(ParentFromChildTransform {
                        parent: TransformFrameIdHash::entity_path_hierarchy_root(),
                        transform: DAffine3::from_translation(glam::dvec3(3.0, 4.0, 5.0)),
                    })
                );
            }

            // Now add a separate chunk with a clear.
            entity_db.add_chunk(&Arc::new(clear_chunk))?;
        } else if clear_in_separate_chunk && first_clear_then_data {
            // First add clear chunk.
            entity_db.add_chunk(&Arc::new(clear_chunk))?;

            // Warm the cache with this situation.
            apply_store_subscriber_events(&mut cache, &entity_db);
            let transforms_per_timeline = cache.transforms_for_timeline(timeline_name);
            assert_eq!(
                transforms_per_timeline
                    .frame_transforms(TransformFrameIdHash::from_entity_path(&path)),
                None
            );

            // And only now add the data chunk.
            entity_db.add_chunk(&Arc::new(data_chunk))?;
        } else {
            let chunk = data_chunk.concatenated(&clear_chunk)?;
            entity_db.add_chunk(&Arc::new(chunk))?;
        }

        // Check transforms AFTER we apply the clear.
        {
            apply_store_subscriber_events(&mut cache, &entity_db);
            let transforms_per_timeline = cache.transforms_for_timeline(timeline_name);
            let transforms = transforms_per_timeline
                .frame_transforms(TransformFrameIdHash::from_entity_path(&path))
                .unwrap();

            assert_eq!(
                transforms.latest_at_transform(&entity_db, &LatestAtQuery::new(timeline_name, 1)),
                Some(ParentFromChildTransform {
                    parent: TransformFrameIdHash::entity_path_hierarchy_root(),
                    transform: DAffine3::from_translation(glam::dvec3(1.0, 2.0, 3.0)),
                })
            );
            assert_eq!(
                transforms.latest_at_transform(&entity_db, &LatestAtQuery::new(timeline_name, 2)),
                None
            );
            assert_eq!(
                transforms.latest_at_transform(&entity_db, &LatestAtQuery::new(timeline_name, 3)),
                Some(ParentFromChildTransform {
                    parent: TransformFrameIdHash::entity_path_hierarchy_root(),
                    transform: DAffine3::from_translation(glam::dvec3(3.0, 4.0, 5.0)),
                })
            );
        }
    }

    Ok(())
}

#[test]
fn test_clear_recursive() -> Result<(), Box<dyn std::error::Error>> {
    for (clear_in_separate_chunk, update_after_each_chunk) in
        [(false, false), (false, true), (true, false), (true, true)]
    {
        println!(
            "clear_in_separate_chunk: {clear_in_separate_chunk}, apply_after_each_chunk: {update_after_each_chunk}",
        );

        let mut entity_db = new_entity_db_with_subscriber_registered();
        let mut cache = TransformResolutionCache::default();

        let timeline = Timeline::new_sequence("t");

        let mut parent_chunk = Chunk::builder(EntityPath::from("parent")).with_archetype_auto_row(
            [(timeline, 1)],
            &Transform3D::from_translation([1.0, 2.0, 3.0]),
        );
        if !clear_in_separate_chunk {
            parent_chunk = parent_chunk
                .with_archetype_auto_row([(timeline, 2)], &archetypes::Clear::new(true));
        }
        entity_db.add_chunk(&Arc::new(parent_chunk.build()?))?;
        if update_after_each_chunk {
            apply_store_subscriber_events(&mut cache, &entity_db);
        }

        let child_chunk = Chunk::builder(EntityPath::from("parent/child")).with_archetype_auto_row(
            [(timeline, 1)],
            &Transform3D::from_translation([1.0, 2.0, 3.0]),
        );
        entity_db.add_chunk(&Arc::new(child_chunk.build()?))?;
        if update_after_each_chunk {
            apply_store_subscriber_events(&mut cache, &entity_db);
        }

        if clear_in_separate_chunk {
            let chunk = Chunk::builder(EntityPath::from("parent"))
                .with_archetype_auto_row([(timeline, 2)], &archetypes::Clear::new(true))
                .build()?;
            entity_db.add_chunk(&Arc::new(chunk))?;
            if update_after_each_chunk {
                apply_store_subscriber_events(&mut cache, &entity_db);
            }
        }

        let timeline = *timeline.name();
        apply_store_subscriber_events(&mut cache, &entity_db);
        let transforms_per_timeline = cache.transforms_for_timeline(timeline);

        for path in [EntityPath::from("parent"), EntityPath::from("parent/child")] {
            let transform = transforms_per_timeline
                .frame_transforms(TransformFrameIdHash::from_entity_path(&path))
                .unwrap();

            println!("checking for correct transforms for path: {path:?}");

            assert_eq!(
                transform.latest_at_transform(&entity_db, &LatestAtQuery::new(timeline, 1)),
                Some(ParentFromChildTransform {
                    parent: TransformFrameIdHash::from_entity_path(&path.parent().unwrap()),
                    transform: DAffine3::from_translation(glam::dvec3(1.0, 2.0, 3.0)),
                })
            );
            assert_eq!(
                transform.latest_at_transform(&entity_db, &LatestAtQuery::new(timeline, 2)),
                None
            );
        }
    }

    Ok(())
}

#[derive(Debug, Clone, Copy)]
enum ChildParentFrameChangesOverTimeTestMode {
    SingleChunk,
    MultipleChunksInOrder,
    MultipleChunksReverseOrder,
}

fn test_single_child_and_parent_over_time(
    mode: ChildParentFrameChangesOverTimeTestMode,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut entity_db = new_entity_db_with_subscriber_registered();
    let mut cache = TransformResolutionCache::default();

    let timeline = Timeline::new_sequence("t");
    let timeline_name = *timeline.name();

    let chunk = Chunk::builder(EntityPath::from("my_entity"))
        .with_archetype_auto_row(
            [(timeline, 1)],
            &Transform3D::from_translation([1.0, 0.0, 0.0]),
        )
        .with_archetype_auto_row(
            [(timeline, 2)],
            &Transform3D::new()
                .with_translation([2.0, 0.0, 0.0])
                .with_child_frame("frame0"), // Uses implicit entity-path derived parent frame.
        )
        .with_archetype_auto_row(
            [(timeline, 3)],
            &Transform3D::new()
                .with_translation([3.0, 0.0, 0.0])
                .with_child_frame("frame0")
                .with_parent_frame("frame1"),
        )
        .with_archetype_auto_row(
            [(timeline, 4)],
            &Transform3D::new()
                .with_translation([4.0, 0.0, 0.0])
                .with_child_frame("frame2")
                .with_parent_frame("frame3"),
        )
        .build()?;

    match mode {
        ChildParentFrameChangesOverTimeTestMode::SingleChunk => {
            entity_db.add_chunk(&Arc::new(chunk))?;
            apply_store_subscriber_events(&mut cache, &entity_db);
        }
        ChildParentFrameChangesOverTimeTestMode::MultipleChunksInOrder => {
            for row_idx in 0..chunk.num_rows() {
                entity_db.add_chunk(&Arc::new(
                    chunk.row_sliced_shallow(row_idx, 1).with_id(ChunkId::new()),
                ))?;
                apply_store_subscriber_events(&mut cache, &entity_db);
            }
        }
        ChildParentFrameChangesOverTimeTestMode::MultipleChunksReverseOrder => {
            for row_idx in (0..chunk.num_rows()).rev() {
                entity_db.add_chunk(&Arc::new(
                    chunk.row_sliced_shallow(row_idx, 1).with_id(ChunkId::new()),
                ))?;
                apply_store_subscriber_events(&mut cache, &entity_db);
            }
        }
    }

    let timeline_transforms = cache.transforms_for_timeline(*timeline.name());

    // State of the implicit frame over time.
    let transforms_implicit_frame = timeline_transforms
        .frame_transforms(TransformFrameIdHash::from_entity_path(&EntityPath::from(
            "my_entity",
        )))
        .unwrap();
    // Nothing we add over time affects the implicit frame whose relationship is set at frame 1
    for t in [1, 2, 3, 4, 5] {
        assert_eq!(
            transforms_implicit_frame
                .latest_at_transform(&entity_db, &LatestAtQuery::new(timeline_name, t)),
            Some(ParentFromChildTransform {
                parent: TransformFrameIdHash::entity_path_hierarchy_root(),
                transform: DAffine3::from_translation(glam::dvec3(1.0, 0.0, 0.0)),
            }),
            "querying at t=={t}"
        );
    }

    // State of frame0 over time.
    let transforms_frame0 = timeline_transforms
        .frame_transforms(TransformFrameIdHash::from_str("frame0"))
        .unwrap();
    for (t, expected_translation_and_parent) in [
        (4, Some((3.0, TransformFrameIdHash::from_str("frame1")))),
        (3, Some((3.0, TransformFrameIdHash::from_str("frame1")))),
        (
            2,
            Some((2.0, TransformFrameIdHash::entity_path_hierarchy_root())),
        ),
        (1, None),
        (0, None),
    ] {
        assert_eq!(
            transforms_frame0
                .latest_at_transform(&entity_db, &LatestAtQuery::new(timeline_name, t)),
            expected_translation_and_parent.map(|(x, parent)| ParentFromChildTransform {
                parent,
                transform: DAffine3::from_translation(glam::dvec3(x, 0.0, 0.0)),
            }),
            "querying at t=={t}"
        );
    }

    // frame1 is never a child, only a parent.
    assert!(
        timeline_transforms
            .frame_transforms(TransformFrameIdHash::from_str("custom_frame1"))
            .is_none(),
    );

    // State of frame2 over time.
    let transforms_frame2 = timeline_transforms
        .frame_transforms(TransformFrameIdHash::from_str("frame2"))
        .unwrap();
    for t in [1, 2, 3] {
        assert_eq!(
            transforms_frame2
                .latest_at_transform(&entity_db, &LatestAtQuery::new(timeline_name, t)),
            None
        );
    }
    for t in [4, 5] {
        assert_eq!(
            transforms_frame2
                .latest_at_transform(&entity_db, &LatestAtQuery::new(timeline_name, t)),
            Some(ParentFromChildTransform {
                parent: TransformFrameIdHash::from_str("frame3"),
                transform: DAffine3::from_translation(glam::dvec3(4.0, 0.0, 0.0)),
            }),
            "querying at t=={t}"
        );
    }

    // frame3 is never a child, only a parent.
    assert!(
        timeline_transforms
            .frame_transforms(TransformFrameIdHash::from_str("custom_frame3"))
            .is_none()
    );

    Ok(())
}

#[test]
fn test_single_child_and_parent_over_time_single_chunk() -> Result<(), Box<dyn std::error::Error>> {
    test_single_child_and_parent_over_time(ChildParentFrameChangesOverTimeTestMode::SingleChunk)
}

#[test]
fn test_single_child_and_parent_over_time_multiple_chunks_in_order()
-> Result<(), Box<dyn std::error::Error>> {
    test_single_child_and_parent_over_time(
        ChildParentFrameChangesOverTimeTestMode::MultipleChunksInOrder,
    )
}

#[test]
fn test_single_child_and_parent_over_time_multiple_chunks_reverse_order()
-> Result<(), Box<dyn std::error::Error>> {
    test_single_child_and_parent_over_time(
        ChildParentFrameChangesOverTimeTestMode::MultipleChunksReverseOrder,
    )
}

#[test]
fn test_static_child_frames() -> Result<(), Box<dyn std::error::Error>> {
    let mut entity_db = new_entity_db_with_subscriber_registered();
    let mut cache = TransformResolutionCache::default();

    let timeline = Timeline::new_sequence("t");
    let timeline_name = *timeline.name();

    let temporal_entity_path = EntityPath::from("my_entity");
    let static_entity_path = EntityPath::from("my_static_entity");

    entity_db.add_chunk(&Arc::new(
        Chunk::builder(static_entity_path.clone())
            .with_archetype_auto_row(
                TimePoint::STATIC,
                &Transform3D::new()
                    .with_translation([1.0, 0.0, 0.0])
                    .with_child_frame("frame0"),
            )
            .build()?,
    ))?;
    entity_db.add_chunk(&Arc::new(
        Chunk::builder(temporal_entity_path)
            .with_archetype_auto_row(
                [(timeline, 1)],
                &Transform3D::new()
                    .with_translation([2.0, 0.0, 0.0])
                    .with_child_frame("frame1"),
            )
            .build()?,
    ))?;
    apply_store_subscriber_events(&mut cache, &entity_db);

    {
        let timeline_transforms = cache.transforms_for_timeline(*timeline.name());

        // Check frame0 only ever sees the static transform.
        let transforms_frame0 = timeline_transforms
            .frame_transforms(TransformFrameIdHash::from_str("frame0"))
            .unwrap();
        assert_eq!(
            transforms_frame0
                .latest_at_transform(&entity_db, &LatestAtQuery::new(timeline_name, 0)),
            Some(ParentFromChildTransform {
                parent: TransformFrameIdHash::entity_path_hierarchy_root(),
                transform: DAffine3::from_translation(glam::dvec3(1.0, 0.0, 0.0)),
            })
        );

        // Check frame1 only ever sees the temporal transform.
        let transforms_frame1 = timeline_transforms
            .frame_transforms(TransformFrameIdHash::from_str("frame1"))
            .unwrap();
        assert_eq!(
            transforms_frame1
                .latest_at_transform(&entity_db, &LatestAtQuery::new(timeline_name, 0)),
            None
        );
        assert_eq!(
            transforms_frame1
                .latest_at_transform(&entity_db, &LatestAtQuery::new(timeline_name, 1)),
            Some(ParentFromChildTransform {
                parent: TransformFrameIdHash::entity_path_hierarchy_root(),
                transform: DAffine3::from_translation(glam::dvec3(2.0, 0.0, 0.0)),
            })
        );
    }

    // Now we change the static chunk to also talk about a new frame2.
    // (Note we're not allowed to also mention frame1 since it is already used by our non-temporal entity)
    // Before, there was a translation there but due to atomic latest-at we won't see that.
    entity_db.add_chunk(&Arc::new(
        Chunk::builder(static_entity_path)
            .with_archetype_auto_row(
                TimePoint::STATIC,
                &Transform3D::new()
                    .with_child_frame("frame2")
                    .with_scale(2.0),
            )
            .build()?,
    ))?;
    apply_store_subscriber_events(&mut cache, &entity_db);

    {
        let timeline_transforms = cache.transforms_for_timeline(*timeline.name());

        // Information about frame0 is still there, just like it would be when adding additional temporal rows at the same time.
        let transforms_frame0 = timeline_transforms
            .frame_transforms(TransformFrameIdHash::from_str("frame0"))
            .unwrap();
        assert_eq!(
            transforms_frame0
                .latest_at_transform(&entity_db, &LatestAtQuery::new(timeline_name, 0)),
            Some(ParentFromChildTransform {
                parent: TransformFrameIdHash::entity_path_hierarchy_root(),
                transform: DAffine3::from_translation(glam::dvec3(1.0, 0.0, 0.0)),
            })
        );

        // But there's also a new frame2.
        let transforms_frame2 = timeline_transforms
            .frame_transforms(TransformFrameIdHash::from_str("frame2"))
            .unwrap();
        assert_eq!(
            transforms_frame2
                .latest_at_transform(&entity_db, &LatestAtQuery::new(timeline_name, 0)),
            Some(ParentFromChildTransform {
                parent: TransformFrameIdHash::entity_path_hierarchy_root(),
                transform: DAffine3::from_scale(glam::DVec3::splat(2.0)),
            })
        );
    }

    Ok(())
}

#[test]
fn test_different_associated_paths_for_static_and_temporal()
-> Result<(), Box<dyn std::error::Error>> {
    let mut entity_db = new_entity_db_with_subscriber_registered();
    let mut cache = TransformResolutionCache::default();

    let timeline = Timeline::new_sequence("t");
    let timeline_name = *timeline.name();

    let static_entity_path = EntityPath::from("static_entity");
    let temporal_entity_path = EntityPath::from("temporal_entity");
    let child_frame = TransformFrameIdHash::from_str("child_frame");

    let static_chunk = Chunk::builder(static_entity_path.clone())
        .with_archetype_auto_row(
            TimePoint::STATIC,
            &Transform3D::new()
                .with_translation([1.0, 2.0, 3.0])
                .with_child_frame("child_frame")
                .with_parent_frame("parent_frame"),
        )
        .build()?;
    let temporal_chunk = Chunk::builder(temporal_entity_path.clone())
        .with_archetype_auto_row(
            [(timeline, 1)],
            &Transform3D::new()
                .with_translation([4.0, 5.0, 6.0])
                .with_child_frame("child_frame")
                .with_parent_frame("parent_frame"),
        )
        .build()?;

    #[derive(Debug)]
    enum Scenario {
        StaticAndTemporalAtOnce,
        StaticFirstThenTemporal,
        TemporalFirstThenStatic,
    }

    for scenario in [
        Scenario::StaticAndTemporalAtOnce,
        Scenario::StaticFirstThenTemporal,
        Scenario::TemporalFirstThenStatic,
    ] {
        match scenario {
            Scenario::StaticAndTemporalAtOnce => {
                entity_db.add_chunk(&Arc::new(static_chunk.clone()))?;
                entity_db.add_chunk(&Arc::new(temporal_chunk.clone()))?;
            }
            Scenario::StaticFirstThenTemporal => {
                entity_db.add_chunk(&Arc::new(static_chunk.clone()))?;
            }
            Scenario::TemporalFirstThenStatic => {
                entity_db.add_chunk(&Arc::new(temporal_chunk.clone()))?;
            }
        }
        apply_store_subscriber_events(&mut cache, &entity_db);

        // Warm cache.
        {
            let transforms_per_timeline = cache.transforms_for_timeline(timeline_name);
            let transforms = transforms_per_timeline
                .frame_transforms(child_frame)
                .unwrap();
            transforms.latest_at_transform(&entity_db, &LatestAtQuery::new(timeline_name, 0));
            transforms.latest_at_transform(&entity_db, &LatestAtQuery::new(timeline_name, 1));
        }

        // Add extra chunk.
        match scenario {
            Scenario::StaticAndTemporalAtOnce => {
                // Already added both.
            }
            Scenario::StaticFirstThenTemporal => {
                entity_db.add_chunk(&Arc::new(temporal_chunk.clone()))?;
            }
            Scenario::TemporalFirstThenStatic => {
                entity_db.add_chunk(&Arc::new(static_chunk.clone()))?;
            }
        }
        apply_store_subscriber_events(&mut cache, &entity_db);

        // Both static and temporal data should be accessible
        let transforms_per_timeline = cache.transforms_for_timeline(timeline_name);
        let transforms = transforms_per_timeline
            .frame_transforms(child_frame)
            .unwrap();

        // At time 0, should see static data
        assert_eq!(
            transforms.latest_at_transform(&entity_db, &LatestAtQuery::new(timeline_name, 0)),
            Some(ParentFromChildTransform {
                parent: TransformFrameIdHash::from_str("parent_frame"),
                transform: DAffine3::from_translation(glam::dvec3(1.0, 2.0, 3.0)),
            }),
            "Unexpected transform at time 0 (scenario: {scenario:?})",
        );
        // At time 1, should see temporal data (overriding static due to atomic-latest-at)
        assert_eq!(
            transforms.latest_at_transform(&entity_db, &LatestAtQuery::new(timeline_name, 1)),
            Some(ParentFromChildTransform {
                parent: TransformFrameIdHash::from_str("parent_frame"),
                transform: DAffine3::from_translation(glam::dvec3(4.0, 5.0, 6.0)),
            }),
            "Unexpected transform at time 1 (scenario: {scenario:?})",
        );

        // Verify associated entity paths are correctly tracked
        assert_eq!(
            transforms.associated_entity_path(TimeInt::STATIC),
            &static_entity_path,
            "Unexpected path for static data (scenario: {scenario:?})",
        );
        assert_eq!(
            transforms.associated_entity_path(TimeInt::new_temporal(1)),
            &temporal_entity_path,
            "Unexpected path for temporal data (scenario: {scenario:?})",
        );

        // Test on a different timeline that never saw the temporal data
        let other_timeline = TimelineName::new("other");
        let transforms_per_timeline = cache.transforms_for_timeline(other_timeline);
        let transforms = transforms_per_timeline
            .frame_transforms(child_frame)
            .unwrap();
        assert_eq!(
            transforms.latest_at_transform(&entity_db, &LatestAtQuery::new(other_timeline, 100)),
            Some(ParentFromChildTransform {
                parent: TransformFrameIdHash::from_str("parent_frame"),
                transform: DAffine3::from_translation(glam::dvec3(1.0, 2.0, 3.0)),
            }),
            "Unexpected transform on other timeline (scenario: {scenario:?})",
        );
    }

    Ok(())
}

#[track_caller]
fn ensure_no_logged_error(rx: &re_log::Receiver<re_log::LogMsg>) {
    if let Ok(msg) = rx.try_recv() {
        panic!("Unexpected log output: {msg:?}");
    }
}

fn test_error_on_changing_associated_path(time: TimeInt) -> Result<(), Box<dyn std::error::Error>> {
    re_log::setup_logging();
    let (logger, log_rx) = re_log::ChannelLogger::new(re_log::LevelFilter::Error);
    re_log::add_boxed_logger(Box::new(logger)).expect("Failed to add logger");

    let mut entity_db = EntityDb::new(StoreInfo::testing().store_id);
    let mut cache = TransformResolutionCache::default();

    let time_point = if time.is_static() {
        TimePoint::STATIC
    } else {
        let timeline = Timeline::new_sequence("t");
        // Initialize the timeline (lazy initialization).
        cache.ensure_timeline_is_initialized(entity_db.storage_engine().store(), *timeline.name());
        [(timeline, time)].into()
    };

    // First, create temporal transform
    let temporal_chunk1 = Chunk::builder(EntityPath::from("entity_a"))
        .with_archetype_auto_row(
            time_point.clone(),
            &Transform3D::from_translation([1.0, 0.0, 0.0]).with_child_frame("my_frame"),
        )
        .build()?;
    cache.process_store_events(entity_db.add_chunk(&Arc::new(temporal_chunk1))?.iter());

    ensure_no_logged_error(&log_rx);

    // Try to associate the same frame with a different temporal entity - should log error
    let temporal_chunk2 = Chunk::builder(EntityPath::from("entity_b"))
        .with_archetype_auto_row(
            time_point,
            &Transform3D::from_translation([2.0, 0.0, 0.0]).with_child_frame("my_frame"),
        )
        .build()?;
    cache.process_store_events(entity_db.add_chunk(&Arc::new(temporal_chunk2))?.iter());

    let error = log_rx.try_recv().unwrap();
    ensure_no_logged_error(&log_rx); // Exactly one error.

    assert_eq!(error.level, re_log::Level::Error);
    assert!(
        error.msg.contains("entity_a"),
        "Expected to mention previous entity, but msg was {}",
        error.msg
    );
    assert!(
        error.msg.contains("entity_b"),
        "Expected to mention new entity, but msg was {}",
        error.msg
    );
    assert!(
        error.msg.contains("my_frame"),
        "Expected to mention target, but msg was {}",
        error.msg
    );

    Ok(())
}

#[test]
fn test_error_on_changing_associated_path_static() -> Result<(), Box<dyn std::error::Error>> {
    test_error_on_changing_associated_path(TimeInt::STATIC)
}

#[test]
fn test_error_on_changing_associated_path_temporal() -> Result<(), Box<dyn std::error::Error>> {
    test_error_on_changing_associated_path(TimeInt::new_temporal(0))
}

#[test]
fn test_pinhole_with_explicit_frames() -> Result<(), Box<dyn std::error::Error>> {
    let mut entity_db = new_entity_db_with_subscriber_registered();
    let mut cache = TransformResolutionCache::default();

    let timeline = Timeline::new_sequence("t");
    let timeline_name = *timeline.name();

    let image_from_camera =
        PinholeProjection::from_focal_length_and_principal_point([1.0, 2.0], [1.0, 2.0]);

    let chunk = Chunk::builder(EntityPath::from("my_entity"))
        // Add pinhole with explicit child and parent frames
        .with_archetype_auto_row(
            [(timeline, 0)],
            &Pinhole::new(image_from_camera)
                .with_child_frame("child_frame")
                .with_parent_frame("parent_frame"),
        )
        // Add a 3D transform on top.
        .with_archetype_auto_row(
            [(timeline, 1)],
            &Transform3D::from_translation([1.0, 2.0, 3.0])
                .with_child_frame("child_frame")
                .with_parent_frame("parent_frame"),
        )
        // Add a 3D transform to a different child frame.
        .with_archetype_auto_row(
            [(timeline, 2)],
            &Transform3D::from_translation([3.0, 4.0, 5.0])
                .with_child_frame("other_frame")
                .with_parent_frame("parent_frame"),
        )
        // Add a pinhole to that same relation, this time with an explicit resolution.
        .with_archetype_auto_row(
            [(timeline, 3)],
            &Pinhole::new(image_from_camera)
                .with_resolution([1.0, 2.0])
                .with_child_frame("other_frame")
                .with_parent_frame("parent_frame"),
        )
        .build()?;
    entity_db.add_chunk(&Arc::new(chunk))?;

    apply_store_subscriber_events(&mut cache, &entity_db);

    let transforms_per_timeline = cache.transforms_for_timeline(timeline_name);

    // Check transforms going out from child_frame
    let transforms = transforms_per_timeline
        .frame_transforms(TransformFrameIdHash::from_str("child_frame"))
        .unwrap();
    for t in [0, 1, 2, 3] {
        // Pinhole from child_frame->X exists at all times unchanged.
        assert_eq!(
            transforms.latest_at_pinhole(&entity_db, &LatestAtQuery::new(timeline_name, t)),
            Some(ResolvedPinholeProjection {
                parent: TransformFrameIdHash::from_str("parent_frame"),
                image_from_camera,
                resolution: None,
                view_coordinates: archetypes::Pinhole::DEFAULT_CAMERA_XYZ,
            }),
            "Unexpected pinhole for child_frame at time t={t}"
        );

        // After time 1 we have a transform on top
        if t == 0 {
            assert_eq!(
                transforms.latest_at_transform(&entity_db, &LatestAtQuery::new(timeline_name, t)),
                None,
                "Unexpected transform for child_frame at time t={t}"
            );
        } else {
            assert_eq!(
                transforms.latest_at_transform(&entity_db, &LatestAtQuery::new(timeline_name, t)),
                Some(ParentFromChildTransform {
                    parent: TransformFrameIdHash::from_str("parent_frame"),
                    transform: DAffine3::from_translation(glam::dvec3(1.0, 2.0, 3.0)),
                }),
                "Unexpected transform for child_frame at time t={t}"
            );
        }
    }

    // Check transforms going out from other_frame
    let transforms = transforms_per_timeline
        .frame_transforms(TransformFrameIdHash::from_str("other_frame"))
        .unwrap();
    for t in [0, 1, 2, 3] {
        // Pinhole from other_frame->X exists only at time t==3
        if t < 3 {
            assert_eq!(
                transforms.latest_at_pinhole(&entity_db, &LatestAtQuery::new(timeline_name, t)),
                None,
                "Unexpected pinhole for other_frame at time t={t}"
            );
        } else {
            assert_eq!(
                transforms.latest_at_pinhole(&entity_db, &LatestAtQuery::new(timeline_name, t)),
                Some(ResolvedPinholeProjection {
                    parent: TransformFrameIdHash::from_str("parent_frame"),
                    image_from_camera,
                    resolution: Some([1.0, 2.0].into()),
                    view_coordinates: archetypes::Pinhole::DEFAULT_CAMERA_XYZ,
                }),
                "Unexpected pinhole for other_frame at time t={t}"
            );
        }

        // After time 2 we have a transform.
        if t < 2 {
            assert_eq!(
                transforms.latest_at_transform(&entity_db, &LatestAtQuery::new(timeline_name, t)),
                None,
                "Unexpected transform for other_frame at time t={t}"
            );
        } else {
            assert_eq!(
                transforms.latest_at_transform(&entity_db, &LatestAtQuery::new(timeline_name, t)),
                Some(ParentFromChildTransform {
                    parent: TransformFrameIdHash::from_str("parent_frame"),
                    transform: DAffine3::from_translation(glam::dvec3(3.0, 4.0, 5.0)),
                }),
                "Unexpected transform for other_frame at time t={t}"
            );
        }
    }

    Ok(())
}

// TODO(andreas): We're missing tests for more corner cases involving child frames and (recursive) clears.

#[test]
fn test_gc() -> Result<(), Box<dyn std::error::Error>> {
    use re_byte_size::SizeBytes as _;

    let mut entity_db = new_entity_db_with_subscriber_registered();
    let mut cache = TransformResolutionCache::default();

    let timeline = Timeline::new_sequence("t");
    let chunk = Chunk::builder(EntityPath::from("my_entity0"))
        .with_archetype_auto_row(
            [(timeline, 1)],
            &Transform3D::from_translation([1.0, 2.0, 3.0]),
        )
        .build()?;
    entity_db.add_chunk(&Arc::new(chunk))?;

    // Apply some updates to the transform before GC pass.
    apply_store_subscriber_events(&mut cache, &entity_db);
    let num_bytes_before_gc = cache.total_size_bytes();

    let chunk = Chunk::builder(EntityPath::from("my_entity1"))
        .with_archetype_auto_row(
            [(timeline, 2)],
            &Transform3D::from_translation([4.0, 5.0, 6.0]),
        )
        .build()?;
    entity_db.add_chunk(&Arc::new(chunk))?;

    // Don't apply updates for this chunk.
    let _store_events = entity_db.gc(&GarbageCollectionOptions::gc_everything());
    apply_store_subscriber_events(&mut cache, &entity_db);
    let num_bytes_after_gc = cache.total_size_bytes();
    assert!(
        num_bytes_after_gc < num_bytes_before_gc,
        "Expected cache size to decrease after GC (before/after: {num_bytes_before_gc} bytes)"
    );

    Ok(())
}

// Tests GCing a recursive clear.
#[test]
fn test_gc_recursive_clear() -> Result<(), Box<dyn std::error::Error>> {
    let mut entity_db = new_entity_db_with_subscriber_registered();
    let mut cache = TransformResolutionCache::default();

    let timeline = Timeline::new_sequence("t");
    let chunk = Chunk::builder(EntityPath::from("my_recursive_clear"))
        .with_archetype_auto_row([(timeline, 1)], &archetypes::Clear::new(true))
        .build()?;
    entity_db.add_chunk(&Arc::new(chunk))?;

    // Apply some updates to the transform before GC pass.
    apply_store_subscriber_events(&mut cache, &entity_db);

    assert!(
        cache
            .transforms_for_timeline(*timeline.name())
            .recursive_clears
            .contains_key(&EntityPath::from("my_recursive_clear")),
    );

    // Don't apply updates for this chunk.
    let _store_events = entity_db.gc(&GarbageCollectionOptions::gc_everything());
    apply_store_subscriber_events(&mut cache, &entity_db);

    assert!(
        cache
            .transforms_for_timeline(*timeline.name())
            .recursive_clears
            .is_empty(),
    );

    Ok(())
}

#[test]
fn test_cache_invalidation() -> Result<(), Box<dyn std::error::Error>> {
    let mut entity_db = EntityDb::new(StoreInfo::testing().store_id);
    let mut cache = TransformResolutionCache::default();

    let timeline = Timeline::new_sequence("t");
    let timeline_name = *timeline.name();
    let frame = TransformFrameIdHash::from_entity_path(&EntityPath::from("my_entity"));

    // Initialize the timeline (lazy initialization).
    cache.ensure_timeline_is_initialized(entity_db.storage_engine().store(), timeline_name);

    // Initial chunk with various events, some of which don't do anything about transforms.
    let chunk = Chunk::builder(EntityPath::from("my_entity"))
        .with_archetype_auto_row(
            [(timeline, 1)],
            &Transform3D::from_translation([1.0, 0.0, 0.0]),
        )
        .with_archetype_auto_row([(timeline, 2)], &MyPoints::new([MyPoint::new(0.0, 0.0)]))
        .with_archetype_auto_row(
            [(timeline, 3)],
            &Transform3D::from_translation([2.0, 0.0, 0.0]),
        )
        .build()?;
    cache.process_store_events(entity_db.add_chunk(&Arc::new(chunk))?.iter());

    {
        // Query all transforms, warming the cache.
        let transforms_per_timeline = cache.transforms_for_timeline(timeline_name);
        let transforms = transforms_per_timeline.frame_transforms(frame).unwrap();
        for (time, expected_translation) in [
            (1, glam::dvec3(1.0, 0.0, 0.0)),
            (2, glam::dvec3(1.0, 0.0, 0.0)),
            (3, glam::dvec3(2.0, 0.0, 0.0)),
        ] {
            assert_eq!(
                transforms
                    .latest_at_transform(&entity_db, &LatestAtQuery::new(timeline_name, time)),
                Some(ParentFromChildTransform {
                    parent: TransformFrameIdHash::entity_path_hierarchy_root(),
                    transform: DAffine3::from_translation(expected_translation),
                }),
                "querying at time {time}"
            );
        }
    }

    // New chunk overriding some of the times and adding new ones.
    let chunk = Chunk::builder(EntityPath::from("my_entity"))
        .with_archetype_auto_row(
            [(timeline, 1)],
            &Transform3D::from_translation([3.0, 0.0, 0.0]),
        )
        .with_archetype_auto_row(
            [(timeline, 2)],
            &Transform3D::from_translation([4.0, 0.0, 0.0]),
        )
        .with_archetype_auto_row(
            [(timeline, 5)],
            &Transform3D::from_translation([5.0, 0.0, 0.0]),
        )
        .build()?;
    cache.process_store_events(entity_db.add_chunk(&Arc::new(chunk))?.iter());

    {
        // Query again, ensuring we get new transforms.
        let transforms_per_timeline = cache.transforms_for_timeline(timeline_name);
        let transforms = transforms_per_timeline.frame_transforms(frame).unwrap();
        for (time, expected_translation) in [
            (1, glam::dvec3(3.0, 0.0, 0.0)),
            (2, glam::dvec3(4.0, 0.0, 0.0)),
            (3, glam::dvec3(2.0, 0.0, 0.0)),
            (4, glam::dvec3(2.0, 0.0, 0.0)),
            (5, glam::dvec3(5.0, 0.0, 0.0)),
        ] {
            assert_eq!(
                transforms
                    .latest_at_transform(&entity_db, &LatestAtQuery::new(timeline_name, time)),
                Some(ParentFromChildTransform {
                    parent: TransformFrameIdHash::entity_path_hierarchy_root(),
                    transform: DAffine3::from_translation(expected_translation),
                }),
                "querying at time {time}"
            );
        }
    }

    // Add a clear chunk.
    let chunk = Chunk::builder(EntityPath::from("my_entity"))
        .with_archetype_auto_row([(timeline, 3)], &archetypes::Clear::new(false))
        .build()?;
    cache.process_store_events(entity_db.add_chunk(&Arc::new(chunk))?.iter());

    {
        // Query again, ensure the transform is cleared in the right places.
        let transforms_per_timeline = cache.transforms_for_timeline(timeline_name);
        let transforms = transforms_per_timeline.frame_transforms(frame).unwrap();
        for (time, expected_translation) in [
            (1, Some(glam::dvec3(3.0, 0.0, 0.0))),
            (2, Some(glam::dvec3(4.0, 0.0, 0.0))),
            (3, None),
            (4, None),
            (5, Some(glam::dvec3(5.0, 0.0, 0.0))),
        ] {
            assert_eq!(
                transforms
                    .latest_at_transform(&entity_db, &LatestAtQuery::new(timeline_name, time)),
                expected_translation.map(|translation| ParentFromChildTransform {
                    parent: TransformFrameIdHash::entity_path_hierarchy_root(),
                    transform: DAffine3::from_translation(translation),
                }),
                "querying at time {time}"
            );
        }
    }

    // Add a chunk that tries to restore the transform _at_ the clear.
    let chunk = Chunk::builder(EntityPath::from("my_entity"))
        .with_archetype_auto_row(
            [(timeline, 3)],
            &Transform3D::from_translation([6.0, 0.0, 0.0]),
        )
        .build()?;
    cache.process_store_events(entity_db.add_chunk(&Arc::new(chunk))?.iter());

    {
        // Query again, ensure that the clear "wins" (no change to before)
        let transforms_per_timeline = cache.transforms_for_timeline(timeline_name);
        let transforms = transforms_per_timeline.frame_transforms(frame).unwrap();
        for (time, expected_translation) in [
            (1, Some(glam::dvec3(3.0, 0.0, 0.0))),
            (2, Some(glam::dvec3(4.0, 0.0, 0.0))),
            (3, None),
            (4, None),
            (5, Some(glam::dvec3(5.0, 0.0, 0.0))),
        ] {
            assert_eq!(
                transforms
                    .latest_at_transform(&entity_db, &LatestAtQuery::new(timeline_name, time)),
                expected_translation.map(|translation| ParentFromChildTransform {
                    parent: TransformFrameIdHash::entity_path_hierarchy_root(),
                    transform: DAffine3::from_translation(translation),
                }),
                "querying at time {time}"
            );
        }
    }

    Ok(())
}
