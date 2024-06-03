// https://github.com/rust-lang/rust-clippy/issues/10011
#![cfg(test)]

use re_data_store::LatestAtQuery;
use re_entity_db::EntityDb;
use re_log_types::{
    example_components::{MyColor, MyIndex, MyPoint},
    DataRow, EntityPath, RowId, StoreId, TimeInt, TimePoint, Timeline,
};
use re_query::PromiseResolver;
use re_types_core::{archetypes::Clear, components::ClearIsRecursive, AsComponents};

// ---

fn query_latest_component<C: re_types_core::Component>(
    db: &EntityDb,
    entity_path: &EntityPath,
    query: &LatestAtQuery,
) -> Option<(TimeInt, RowId, C)> {
    re_tracing::profile_function!();

    let resolver = PromiseResolver::default();

    let results = db
        .query_caches()
        .latest_at(db.store(), query, entity_path, [C::name()]);
    let results = results.get_required(C::name()).ok()?;

    let &(data_time, row_id) = results.index();
    let data = results.dense::<C>(&resolver)?.first().cloned()?;

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
        let row = DataRow::from_component_batches(
            row_id,
            timepoint,
            entity_path_parent.clone(),
            [&[point] as _, &[color] as _],
        )?;

        db.add_data_row(row)?;

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
        let row = DataRow::from_component_batches(
            row_id,
            timepoint,
            entity_path_child1.clone(),
            [&[point] as _],
        )?;

        db.add_data_row(row)?;

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
        let row = DataRow::from_component_batches(
            row_id,
            timepoint,
            entity_path_child2.clone(),
            [&[color] as _],
        )?;

        db.add_data_row(row)?;

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
        let row = DataRow::from_component_batches(
            row_id,
            timepoint,
            entity_path_parent.clone(),
            clear.as_component_batches().iter().map(|b| b.as_ref()),
        )?;

        db.add_data_row(row)?;

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
        let row = DataRow::from_component_batches(
            row_id,
            timepoint,
            entity_path_parent.clone(),
            clear.as_component_batches().iter().map(|b| b.as_ref()),
        )?;

        db.add_data_row(row)?;

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
        let row = DataRow::from_component_batches(
            row_id,
            timepoint,
            entity_path_parent.clone(),
            [&[instance] as _],
        )?;

        db.add_data_row(row)?;

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
        let row = DataRow::from_component_batches(
            row_id,
            timepoint,
            entity_path_child1.clone(),
            [&[point] as _, &[color] as _],
        )?;

        db.add_data_row(row)?;

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
        let row = DataRow::from_component_batches(
            row_id,
            timepoint,
            entity_path_child2.clone(),
            [&[color] as _, &[point] as _],
        )?;

        db.add_data_row(row)?;

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
        let row = DataRow::from_component_batches(
            row_id,
            timepoint,
            entity_path_grandchild.clone(),
            [&[color] as _],
        )?;

        db.add_data_row(row)?;

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
