use re_data_store::*;
use re_log_types::*;

#[test]
fn test_timeless_data() {
    fn insert_timeless(log_db: &mut LogDb, data_path: &DataPath, what: &str) {
        log_db.add(
            DataMsg {
                msg_id: MsgId::random(),
                time_point: TimePoint::timeless(),
                data_path: data_path.clone(),
                data: Data::String(what.into()).into(),
            }
            .into(),
        );
    }

    fn insert_at_time(
        log_db: &mut LogDb,
        data_path: &DataPath,
        what: &str,
        timeline: TimeSource,
        time: i64,
    ) {
        let mut time_point = TimePoint::default();
        time_point.0.insert(timeline, TimeInt::from(time));

        log_db.add(
            DataMsg {
                msg_id: MsgId::random(),
                time_point,
                data_path: data_path.clone(),
                data: Data::String(what.into()).into(),
            }
            .into(),
        );
    }

    fn query_time_and_data(
        store: &LogDb,
        timeline: &TimeSource,
        data_path: &DataPath,
        query_time: i64,
    ) -> String {
        let (time_msgid_multiindex, data) = store
            .data_store
            .query_data_path(timeline, &TimeQuery::LatestAt(query_time), data_path)
            .unwrap()
            .unwrap();
        assert_eq!(time_msgid_multiindex.len(), 1);

        if let DataVec::String(strings) = data {
            assert_eq!(strings.len(), 1);
            strings[0].clone()
        } else {
            panic!()
        }
    }

    let mut log_db = LogDb::default();

    let data_path_foo = DataPath::new(obj_path!("point"), FieldName::new("pos"));
    let data_path_badger = DataPath::new(obj_path!("point"), FieldName::new("badger"));
    let timeline_a = TimeSource::new("timeline_a", TimeType::Sequence);
    let timeline_b = TimeSource::new("timeline_b", TimeType::Sequence);

    insert_timeless(&mut log_db, &data_path_foo, "timeless__foo__first");
    insert_at_time(
        &mut log_db,
        &data_path_badger,
        "timeline_a__badger__666",
        timeline_a,
        666,
    );
    assert_eq!(
        query_time_and_data(&log_db, &timeline_a, &data_path_foo, 666),
        "timeless__foo__first",
        "Previous timeless data should have been added to the new timeline_a"
    );
    assert_eq!(
        query_time_and_data(&log_db, &timeline_a, &data_path_badger, 666),
        "timeline_a__badger__666",
        "we should find the new data"
    );

    insert_at_time(
        &mut log_db,
        &data_path_foo,
        "timeline_a__foo__666",
        timeline_a,
        666,
    );
    assert_eq!(
        query_time_and_data(&log_db, &timeline_a, &data_path_foo, 42),
        "timeless__foo__first",
        "We should still be able to find the timeless data when looking back in time"
    );
    assert_eq!(
        query_time_and_data(&log_db, &timeline_a, &data_path_foo, 666),
        "timeline_a__foo__666",
        "Timefull data should be findable in the future"
    );

    insert_timeless(&mut log_db, &data_path_foo, "timeless__foo__second");
    assert_eq!(
        query_time_and_data(&log_db, &timeline_a, &data_path_foo, 42),
        "timeless__foo__second",
        "We should be able to update timeless data"
    );
    assert_eq!(
        query_time_and_data(&log_db, &timeline_a, &data_path_foo, 666),
        "timeline_a__foo__666",
        "Timefull data should be findable in the future"
    );

    insert_at_time(
        &mut log_db,
        &data_path_badger,
        "timeline_b__badger__666",
        timeline_b,
        666,
    );
    assert_eq!(
        query_time_and_data(&log_db, &timeline_b, &data_path_foo, 666),
        "timeless__foo__second",
        "Previous timeless data should have been added to the new timeline_b"
    );
}
