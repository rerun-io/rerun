use re_data_store::{DataStoreStats, LatestAtQuery};
use re_entity_db::EntityDb;
use re_log_types::{
    example_components::{MyColor, MyPoint},
    DataRow, EntityPath, RowId, StoreId, TimePoint, Timeline,
};
use re_types_core::{
    archetypes::Clear,
    components::{ClearIsRecursive, InstanceKey},
    AsComponents,
};

/// Complete test suite for the clear & pending clear paths.
#[test]
fn clears() -> anyhow::Result<()> {
    re_log::setup_native_logging();

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
        let timepoint = TimePoint::from_iter([(timeline_frame, 10.into())]);
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
            let query = LatestAtQuery {
                timeline: timeline_frame,
                at: 11.into(),
            };

            let got_point = db
                .store()
                .query_latest_component::<MyPoint>(&entity_path_parent, &query)
                .unwrap()
                .value;
            let got_color = db
                .store()
                .query_latest_component::<MyColor>(&entity_path_parent, &query)
                .unwrap()
                .value;

            similar_asserts::assert_eq!(point, got_point);
            similar_asserts::assert_eq!(color, got_color);
        }
    }

    // * Insert a 2D point for 'child1' at frame #10.
    // * Query 'child1' at frame #11 and make sure we find everything back.
    {
        let row_id = RowId::new();
        let timepoint = TimePoint::from_iter([(timeline_frame, 10.into())]);
        let point = MyPoint::new(42.0, 43.0);
        let row = DataRow::from_component_batches(
            row_id,
            timepoint,
            entity_path_child1.clone(),
            [&[point] as _],
        )?;

        db.add_data_row(row)?;

        {
            let query = LatestAtQuery {
                timeline: timeline_frame,
                at: 11.into(),
            };

            let got_point = db
                .store()
                .query_latest_component::<MyPoint>(&entity_path_child1, &query)
                .unwrap()
                .value;

            similar_asserts::assert_eq!(point, got_point);
        }
    }

    // * Insert a color for 'child2' at frame #10.
    // * Query 'child2' at frame #11 and make sure we find everything back.
    {
        let row_id = RowId::new();
        let timepoint = TimePoint::from_iter([(timeline_frame, 10.into())]);
        let color = MyColor::from(0x00AA00DD);
        let row = DataRow::from_component_batches(
            row_id,
            timepoint,
            entity_path_child2.clone(),
            [&[color] as _],
        )?;

        db.add_data_row(row)?;

        {
            let query = LatestAtQuery {
                timeline: timeline_frame,
                at: 11.into(),
            };

            let got_color = db
                .store()
                .query_latest_component::<MyColor>(&entity_path_child2, &query)
                .unwrap()
                .value;

            similar_asserts::assert_eq!(color, got_color);
        }
    }

    // * Clear (flat) 'parent' at frame #10.
    // * Query 'parent' at frame #11 and make sure we find nothing.
    // * Query 'child1' at frame #11 and make sure we find everything back.
    // * Query 'child2' at frame #11 and make sure we find everything back.
    {
        let row_id = RowId::new();
        let timepoint = TimePoint::from_iter([(timeline_frame, 10.into())]);
        let clear = Clear::flat();
        let row = DataRow::from_component_batches(
            row_id,
            timepoint,
            entity_path_parent.clone(),
            clear.as_component_batches().iter().map(|b| b.as_ref()),
        )?;

        db.add_data_row(row)?;

        {
            let query = LatestAtQuery {
                timeline: timeline_frame,
                at: 11.into(),
            };

            // parent
            assert!(db
                .store()
                .query_latest_component::<MyPoint>(&entity_path_parent, &query)
                .is_none());
            assert!(db
                .store()
                .query_latest_component::<MyColor>(&entity_path_parent, &query)
                .is_none());
            // the `Clear` component itself doesn't get cleared!
            let got_clear = db
                .store()
                .query_latest_component::<ClearIsRecursive>(&entity_path_parent, &query)
                .unwrap()
                .value;
            similar_asserts::assert_eq!(clear.is_recursive, got_clear);

            // child1
            assert!(db
                .store()
                .query_latest_component::<MyPoint>(&entity_path_child1, &query)
                .is_some());

            // child2
            assert!(db
                .store()
                .query_latest_component::<MyColor>(&entity_path_child2, &query)
                .is_some());
        }
    }

    // * Clear (recursive) 'parent' at frame #10.
    // * Query 'parent' at frame #11 and make sure we find nothing.
    // * Query 'child1' at frame #11 and make sure we find nothing.
    // * Query 'child2' at frame #11 and make sure we find nothing.
    {
        let row_id = RowId::new();
        let timepoint = TimePoint::from_iter([(timeline_frame, 10.into())]);
        let clear = Clear::recursive();
        let row = DataRow::from_component_batches(
            row_id,
            timepoint,
            entity_path_parent.clone(),
            clear.as_component_batches().iter().map(|b| b.as_ref()),
        )?;

        db.add_data_row(row)?;

        {
            let query = LatestAtQuery {
                timeline: timeline_frame,
                at: 11.into(),
            };

            // parent
            assert!(db
                .store()
                .query_latest_component::<MyPoint>(&entity_path_parent, &query)
                .is_none());
            assert!(db
                .store()
                .query_latest_component::<MyColor>(&entity_path_parent, &query)
                .is_none());
            // the `Clear` component itself doesn't get cleared!
            let got_clear = db
                .store()
                .query_latest_component::<ClearIsRecursive>(&entity_path_parent, &query)
                .unwrap()
                .value;
            similar_asserts::assert_eq!(clear.is_recursive, got_clear);

            // child1
            assert!(db
                .store()
                .query_latest_component::<MyPoint>(&entity_path_child1, &query)
                .is_none());

            // child2
            assert!(db
                .store()
                .query_latest_component::<MyColor>(&entity_path_child2, &query)
                .is_none());
        }
    }

    // * Insert an instance key for 'parent' at frame #9.
    // * Query 'parent' at frame #9 and make sure we find it back.
    // * Query 'parent' at frame #11 and make sure we do _not_ find it.
    {
        let row_id = RowId::new();
        let timepoint = TimePoint::from_iter([(timeline_frame, 9.into())]);
        let instance_key = InstanceKey(0);
        let row = DataRow::from_component_batches(
            row_id,
            timepoint,
            entity_path_parent.clone(),
            [&[instance_key] as _],
        )?;

        db.add_data_row(row)?;

        {
            let query = LatestAtQuery {
                timeline: timeline_frame,
                at: 9.into(),
            };

            let got_instance_key = db
                .store()
                .query_latest_component::<InstanceKey>(&entity_path_parent, &query)
                .unwrap()
                .value;
            similar_asserts::assert_eq!(instance_key, got_instance_key);
        }

        {
            let query = LatestAtQuery {
                timeline: timeline_frame,
                at: 11.into(),
            };

            assert!(db
                .store()
                .query_latest_component::<InstanceKey>(&entity_path_parent, &query)
                .is_none());
        }
    }

    // * Insert a 2D point for 'child1' at frame #9.
    // * Insert a color for 'child1' at frame #9.
    // * Query 'child1' at frame #9 and make sure we find everything back.
    // * Query 'child1' at frame #11 and make sure we do _not_ find anything.
    {
        let row_id = RowId::new();
        let timepoint = TimePoint::from_iter([(timeline_frame, 9.into())]);
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
            let query = LatestAtQuery {
                timeline: timeline_frame,
                at: 9.into(),
            };

            let got_point = db
                .store()
                .query_latest_component::<MyPoint>(&entity_path_child1, &query)
                .unwrap()
                .value;
            let got_color = db
                .store()
                .query_latest_component::<MyColor>(&entity_path_child1, &query)
                .unwrap()
                .value;

            similar_asserts::assert_eq!(point, got_point);
            similar_asserts::assert_eq!(color, got_color);
        }

        {
            let query = LatestAtQuery {
                timeline: timeline_frame,
                at: 11.into(),
            };

            assert!(db
                .store()
                .query_latest_component::<MyPoint>(&entity_path_child1, &query)
                .is_none());
            assert!(db
                .store()
                .query_latest_component::<MyColor>(&entity_path_child1, &query)
                .is_none());
        }
    }

    // * Insert a color for 'child2' at frame #9.
    // * Insert a 2D point for 'child2' at frame #9.
    // * Query 'child2' at frame #9 and make sure we find everything back.
    // * Query 'child2' at frame #11 and make sure we do _not_ find anything.
    {
        let row_id = RowId::new();
        let timepoint = TimePoint::from_iter([(timeline_frame, 9.into())]);
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
            let query = LatestAtQuery {
                timeline: timeline_frame,
                at: 9.into(),
            };

            let got_color = db
                .store()
                .query_latest_component::<MyColor>(&entity_path_child2, &query)
                .unwrap()
                .value;
            let got_point = db
                .store()
                .query_latest_component::<MyPoint>(&entity_path_child2, &query)
                .unwrap()
                .value;

            similar_asserts::assert_eq!(color, got_color);
            similar_asserts::assert_eq!(point, got_point);
        }

        {
            let query = LatestAtQuery {
                timeline: timeline_frame,
                at: 11.into(),
            };

            assert!(db
                .store()
                .query_latest_component::<MyColor>(&entity_path_child2, &query)
                .is_none());
            assert!(db
                .store()
                .query_latest_component::<MyPoint>(&entity_path_child2, &query)
                .is_none());
        }
    }

    // * Insert a color for 'grandchild' (new!) at frame #9.
    // * Query 'grandchild' at frame #9 and make sure we find everything back.
    // * Query 'grandchild' at frame #11 and make sure we do _not_ find anything.
    {
        let row_id = RowId::new();
        let timepoint = TimePoint::from_iter([(timeline_frame, 9.into())]);
        let color = MyColor::from(0x00AA00DD);
        let row = DataRow::from_component_batches(
            row_id,
            timepoint,
            entity_path_grandchild.clone(),
            [&[color] as _],
        )?;

        db.add_data_row(row)?;

        {
            let query = LatestAtQuery {
                timeline: timeline_frame,
                at: 9.into(),
            };

            let got_color = db
                .store()
                .query_latest_component::<MyColor>(&entity_path_grandchild, &query)
                .unwrap()
                .value;

            similar_asserts::assert_eq!(color, got_color);
        }

        {
            let query = LatestAtQuery {
                timeline: timeline_frame,
                at: 11.into(),
            };

            assert!(db
                .store()
                .query_latest_component::<MyColor>(&entity_path_grandchild, &query)
                .is_none());
        }
    }

    Ok(())
}

/// Test for GC behavior following clear. This functionality is expected by blueprints.
#[test]
fn clear_and_gc() -> anyhow::Result<()> {
    re_log::setup_native_logging();

    let mut db = EntityDb::new(StoreId::random(re_log_types::StoreKind::Recording));

    let timepoint = TimePoint::timeless();
    let entity_path: EntityPath = "space_view".into();

    // Insert a component, then clear it, then GC.
    {
        // EntityTree is Empty when we start
        assert_eq!(db.tree().num_children_and_fields(), 0);

        let point = MyPoint::new(1.0, 2.0);

        let row = DataRow::from_component_batches(
            RowId::new(),
            timepoint.clone(),
            entity_path.clone(),
            [&[point] as _],
        )?;

        db.add_data_row(row)?;

        db.gc_everything_but_the_latest_row();

        let stats = DataStoreStats::from_store(db.store());
        assert_eq!(stats.timeless.num_rows, 1);

        let clear = DataRow::from_component_batches(
            RowId::new(),
            timepoint.clone(),
            entity_path.clone(),
            Clear::recursive()
                .as_component_batches()
                .iter()
                .map(|b| b.as_ref()),
        )?;

        db.add_data_row(clear)?;

        db.gc_everything_but_the_latest_row();

        // No rows should remain because the table should have been purged
        let stats = DataStoreStats::from_store(db.store());
        assert_eq!(stats.timeless.num_rows, 0);

        // EntityTree should be empty again when we end since everything was GC'd
        assert_eq!(db.tree().num_children_and_fields(), 0);
    }

    Ok(())
}
