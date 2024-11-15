// https://github.com/rust-lang/rust-clippy/issues/10011
#![cfg(test)]

use std::sync::Arc;

use re_chunk::{Chunk, RowId};
use re_chunk_store::LatestAtQuery;
use re_entity_db::EntityDb;
use re_log_types::{
    example_components::{MyColor, MyIndex, MyPoint},
    EntityPath, StoreId, TimeInt, TimePoint, Timeline,
};
use re_types_core::{archetypes::Clear, components::ClearIsRecursive, AsComponents};

// ---

fn query_latest_component<C: re_types_core::Component>(
    db: &EntityDb,
    entity_path: &EntityPath,
    query: &LatestAtQuery,
) -> Option<(TimeInt, RowId, C)> {
    re_tracing::profile_function!();

    let results = db
        .storage_engine()
        .cache()
        .latest_at(query, entity_path, [C::name()]);

    let (data_time, row_id) = results.index();
    let data = results.component_mono::<C>()?;

    Some((data_time, row_id, data))
}

/// Complete test suite for the clear & pending clear paths.
#[test]
fn clears() -> anyhow::Result<()> {
    re_log::setup_logging();

    let mut db = EntityDb::new(StoreId::random(re_log_types::StoreKind::Recording));

    let timeline_frame = Timeline::new_sequence("frame");

    let entity_path_parent: EntityPath = "parent".into();
    let entity_path_child1: EntityPath = "parent/child1".into();
    let entity_path_child2: EntityPath = "parent/deep/deep/down/child2".into();
    let entity_path_grandchild: EntityPath = "parent/child1/grandchild".into();

    // * Insert a 2D point & color for 'parent' at frame #10.
    // * Query 'parent' at frame #11 and make sure we find everything back.
    {
        let row_id = RowId::new();
        let timepoint = TimePoint::from_iter([(timeline_frame, 10)]);
        let point = MyPoint::new(1.0, 2.0);
        let color = MyColor::from(0xFF0000FF);
        let chunk = Chunk::builder(entity_path_parent.clone())
            .with_component_batches(row_id, timepoint, [&[point] as _, &[color] as _])
            .build()?;

        db.add_chunk(&Arc::new(chunk))?;

        {
            let query = LatestAtQuery::new(timeline_frame, 11);
            let (_, _, got_point) =
                query_latest_component::<MyPoint>(&db, &entity_path_parent, &query).unwrap();
            let (_, _, got_color) =
                query_latest_component::<MyColor>(&db, &entity_path_parent, &query).unwrap();

            similar_asserts::assert_eq!(point, got_point);
            similar_asserts::assert_eq!(color, got_color);
        }
    }

    // * Insert a 2D point for 'child1' at frame #10.
    // * Query 'child1' at frame #11 and make sure we find everything back.
    {
        let row_id = RowId::new();
        let timepoint = TimePoint::from_iter([(timeline_frame, 10)]);
        let point = MyPoint::new(42.0, 43.0);
        let chunk = Chunk::builder(entity_path_child1.clone())
            .with_component_batches(row_id, timepoint, [&[point] as _])
            .build()?;

        db.add_chunk(&Arc::new(chunk))?;

        {
            let query = LatestAtQuery::new(timeline_frame, 11);
            let (_, _, got_point) =
                query_latest_component::<MyPoint>(&db, &entity_path_child1, &query).unwrap();

            similar_asserts::assert_eq!(point, got_point);
        }
    }

    // * Insert a color for 'child2' at frame #10.
    // * Query 'child2' at frame #11 and make sure we find everything back.
    {
        let row_id = RowId::new();
        let timepoint = TimePoint::from_iter([(timeline_frame, 10)]);
        let color = MyColor::from(0x00AA00DD);
        let chunk = Chunk::builder(entity_path_child2.clone())
            .with_component_batches(row_id, timepoint, [&[color] as _])
            .build()?;

        db.add_chunk(&Arc::new(chunk))?;

        {
            let query = LatestAtQuery::new(timeline_frame, 11);
            let (_, _, got_color) =
                query_latest_component::<MyColor>(&db, &entity_path_child2, &query).unwrap();

            similar_asserts::assert_eq!(color, got_color);
        }
    }

    // * Clear (flat) 'parent' at frame #10.
    // * Query 'parent' at frame #11 and make sure we find nothing.
    // * Query 'child1' at frame #11 and make sure we find everything back.
    // * Query 'child2' at frame #11 and make sure we find everything back.
    {
        let row_id = RowId::new();
        let timepoint = TimePoint::from_iter([(timeline_frame, 10)]);
        let clear = Clear::flat();
        let chunk = Chunk::builder(entity_path_parent.clone())
            .with_component_batches(
                row_id,
                timepoint,
                clear.as_component_batches().iter().map(|b| b.as_ref()),
            )
            .build()?;

        db.add_chunk(&Arc::new(chunk))?;

        {
            let query = LatestAtQuery::new(timeline_frame, 11);

            // parent
            assert!(query_latest_component::<MyPoint>(&db, &entity_path_parent, &query).is_none());
            assert!(query_latest_component::<MyColor>(&db, &entity_path_parent, &query).is_none());
            // the `Clear` component itself doesn't get cleared!
            let (_, _, got_clear) =
                query_latest_component::<ClearIsRecursive>(&db, &entity_path_parent, &query)
                    .unwrap();
            similar_asserts::assert_eq!(clear.is_recursive, got_clear);

            // child1
            assert!(query_latest_component::<MyPoint>(&db, &entity_path_child1, &query).is_some());

            // child2
            assert!(query_latest_component::<MyColor>(&db, &entity_path_child2, &query).is_some());
        }
    }

    // * Clear (recursive) 'parent' at frame #10.
    // * Query 'parent' at frame #11 and make sure we find nothing.
    // * Query 'child1' at frame #11 and make sure we find nothing.
    // * Query 'child2' at frame #11 and make sure we find nothing.
    {
        let row_id = RowId::new();
        let timepoint = TimePoint::from_iter([(timeline_frame, 10)]);
        let clear = Clear::recursive();
        let chunk = Chunk::builder(entity_path_parent.clone())
            .with_component_batches(
                row_id,
                timepoint,
                clear.as_component_batches().iter().map(|b| b.as_ref()),
            )
            .build()?;

        db.add_chunk(&Arc::new(chunk))?;

        {
            let query = LatestAtQuery::new(timeline_frame, 11);

            // parent
            assert!(query_latest_component::<MyPoint>(&db, &entity_path_parent, &query).is_none());
            assert!(query_latest_component::<MyColor>(&db, &entity_path_parent, &query).is_none());
            // the `Clear` component itself doesn't get cleared!
            let (_, _, got_clear) =
                query_latest_component::<ClearIsRecursive>(&db, &entity_path_parent, &query)
                    .unwrap();
            similar_asserts::assert_eq!(clear.is_recursive, got_clear);

            // child1
            assert!(query_latest_component::<MyPoint>(&db, &entity_path_child1, &query).is_none());

            // child2
            assert!(query_latest_component::<MyColor>(&db, &entity_path_child2, &query).is_none());
        }
    }

    // * Insert an instance key for 'parent' at frame #9.
    // * Query 'parent' at frame #9 and make sure we find it back.
    // * Query 'parent' at frame #11 and make sure we do _not_ find it.
    {
        let row_id = RowId::new();
        let timepoint = TimePoint::from_iter([(timeline_frame, 9)]);
        let instance = MyIndex(0);
        let chunk = Chunk::builder(entity_path_parent.clone())
            .with_component_batches(row_id, timepoint, [&[instance] as _])
            .build()?;

        db.add_chunk(&Arc::new(chunk))?;

        {
            let query = LatestAtQuery::new(timeline_frame, 9);
            let (_, _, got_instance) =
                query_latest_component::<MyIndex>(&db, &entity_path_parent, &query).unwrap();
            similar_asserts::assert_eq!(instance, got_instance);
        }

        {
            let query = LatestAtQuery::new(timeline_frame, 11);
            assert!(query_latest_component::<MyIndex>(&db, &entity_path_parent, &query).is_none());
        }
    }

    // * Insert a 2D point for 'child1' at frame #9.
    // * Insert a color for 'child1' at frame #9.
    // * Query 'child1' at frame #9 and make sure we find everything back.
    // * Query 'child1' at frame #11 and make sure we do _not_ find anything.
    {
        let row_id = RowId::new();
        let timepoint = TimePoint::from_iter([(timeline_frame, 9)]);
        let point = MyPoint::new(42.0, 43.0);
        let color = MyColor::from(0xBBBBBBBB);
        let chunk = Chunk::builder(entity_path_child1.clone())
            .with_component_batches(row_id, timepoint, [&[point] as _, &[color] as _])
            .build()?;

        db.add_chunk(&Arc::new(chunk))?;

        {
            let query = LatestAtQuery::new(timeline_frame, 9);
            let (_, _, got_point) =
                query_latest_component::<MyPoint>(&db, &entity_path_child1, &query).unwrap();
            let (_, _, got_color) =
                query_latest_component::<MyColor>(&db, &entity_path_child1, &query).unwrap();

            similar_asserts::assert_eq!(point, got_point);
            similar_asserts::assert_eq!(color, got_color);
        }

        {
            let query = LatestAtQuery::new(timeline_frame, 11);
            assert!(query_latest_component::<MyPoint>(&db, &entity_path_child1, &query).is_none());
            assert!(query_latest_component::<MyColor>(&db, &entity_path_child1, &query).is_none());
        }
    }

    // * Insert a color for 'child2' at frame #9.
    // * Insert a 2D point for 'child2' at frame #9.
    // * Query 'child2' at frame #9 and make sure we find everything back.
    // * Query 'child2' at frame #11 and make sure we do _not_ find anything.
    {
        let row_id = RowId::new();
        let timepoint = TimePoint::from_iter([(timeline_frame, 9)]);
        let color = MyColor::from(0x00AA00DD);
        let point = MyPoint::new(66.0, 666.0);
        let chunk = Chunk::builder(entity_path_child2.clone())
            .with_component_batches(row_id, timepoint, [&[color] as _, &[point] as _])
            .build()?;

        db.add_chunk(&Arc::new(chunk))?;

        {
            let query = LatestAtQuery::new(timeline_frame, 9);
            let (_, _, got_point) =
                query_latest_component::<MyPoint>(&db, &entity_path_child2, &query).unwrap();
            let (_, _, got_color) =
                query_latest_component::<MyColor>(&db, &entity_path_child2, &query).unwrap();

            similar_asserts::assert_eq!(color, got_color);
            similar_asserts::assert_eq!(point, got_point);
        }

        {
            let query = LatestAtQuery::new(timeline_frame, 11);
            assert!(query_latest_component::<MyPoint>(&db, &entity_path_child2, &query).is_none());
            assert!(query_latest_component::<MyColor>(&db, &entity_path_child2, &query).is_none());
        }
    }

    // * Insert a color for 'grandchild' (new!) at frame #9.
    // * Query 'grandchild' at frame #9 and make sure we find everything back.
    // * Query 'grandchild' at frame #11 and make sure we do _not_ find anything.
    {
        let row_id = RowId::new();
        let timepoint = TimePoint::from_iter([(timeline_frame, 9)]);
        let color = MyColor::from(0x00AA00DD);
        let chunk = Chunk::builder(entity_path_grandchild.clone())
            .with_component_batches(row_id, timepoint, [&[color] as _])
            .build()?;

        db.add_chunk(&Arc::new(chunk))?;

        {
            let query = LatestAtQuery::new(timeline_frame, 9);
            let (_, _, got_color) =
                query_latest_component::<MyColor>(&db, &entity_path_grandchild, &query).unwrap();

            similar_asserts::assert_eq!(color, got_color);
        }

        {
            let query = LatestAtQuery::new(timeline_frame, 11);
            assert!(
                query_latest_component::<MyColor>(&db, &entity_path_grandchild, &query).is_none()
            );
        }
    }

    Ok(())
}

#[test]
fn clears_respect_index_order() -> anyhow::Result<()> {
    re_log::setup_logging();

    let mut db = EntityDb::new(StoreId::random(re_log_types::StoreKind::Recording));

    let timeline_frame = Timeline::new_sequence("frame");

    let entity_path: EntityPath = "parent".into();

    let row_id1 = RowId::new();
    let row_id2 = row_id1.next();
    let row_id3 = row_id2.next();

    let timepoint = TimePoint::from_iter([(timeline_frame, 10)]);

    let point = MyPoint::new(1.0, 2.0);
    let chunk = Chunk::builder(entity_path.clone())
        .with_component_batches(row_id2, timepoint.clone(), [&[point] as _])
        .build()?;

    db.add_chunk(&Arc::new(chunk))?;

    {
        let query = LatestAtQuery::new(timeline_frame, 11);
        let (_, _, got_point) =
            query_latest_component::<MyPoint>(&db, &entity_path, &query).unwrap();
        similar_asserts::assert_eq!(point, got_point);
    }

    let clear = Clear::recursive();
    let chunk = Chunk::builder(entity_path.clone())
        .with_component_batches(
            row_id1, // older row id!
            timepoint.clone(),
            clear.as_component_batches().iter().map(|b| b.as_ref()),
        )
        .build()?;

    db.add_chunk(&Arc::new(chunk))?;

    {
        let query = LatestAtQuery::new(timeline_frame, 11);

        let (_, _, got_point) =
            query_latest_component::<MyPoint>(&db, &entity_path, &query).unwrap();
        similar_asserts::assert_eq!(point, got_point);

        // the `Clear` component itself doesn't get cleared!
        let (_, _, got_clear) =
            query_latest_component::<ClearIsRecursive>(&db, &entity_path, &query).unwrap();
        similar_asserts::assert_eq!(clear.is_recursive, got_clear);
    }

    let clear = Clear::recursive();
    let chunk = Chunk::builder(entity_path.clone())
        .with_component_batches(
            row_id3, // newer row id!
            timepoint.clone(),
            clear.as_component_batches().iter().map(|b| b.as_ref()),
        )
        .build()?;

    db.add_chunk(&Arc::new(chunk))?;

    {
        let query = LatestAtQuery::new(timeline_frame, 11);

        assert!(query_latest_component::<MyPoint>(&db, &entity_path, &query).is_none());

        // the `Clear` component itself doesn't get cleared!
        let (_, _, got_clear) =
            query_latest_component::<ClearIsRecursive>(&db, &entity_path, &query).unwrap();
        similar_asserts::assert_eq!(clear.is_recursive, got_clear);
    }

    Ok(())
}
