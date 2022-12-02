use std::time::{Duration, SystemTime};

use crate::{tests::*, DataStore, TimeQuery};

use re_log_types::{ObjPath as EntityPath, TimeType, Timeline};

#[test]
fn single_entity_multi_timelines_multi_components_roundtrip() {
    let mut store = DataStore::default();

    let ent_path = EntityPath::from("this/that");

    let now = SystemTime::now();
    let now_minus_10ms = now - Duration::from_millis(10);
    let now_minus_20ms = now - Duration::from_millis(20);
    // let now_plus_10ms = now + Duration::from_millis(10);
    let now_plus_20ms = now + Duration::from_millis(20);

    // TODO(cmc): test holes!
    let frame41 = 41;
    let frame42 = 42;
    let frame43 = 43;

    // TODO(cmc): play with differing nb_instances inbetween inserts
    let nb_instances = 3;

    let expected_rects = build_rects(nb_instances);
    let (schema, components) = build_message(
        &ent_path,
        [build_log_time(now_minus_20ms), build_frame_nr(frame43)],
        [expected_rects.clone()],
    );
    eprintln!("inserting into '{ent_path}':\nschema: {schema:#?}\ncomponents: {components:#?}");
    // eprintln!("---\ninserting into '{ent_path}': [log_time, frame_nr], [rects]");
    store.insert(&schema, &components).unwrap();
    // eprintln!("{store}");

    let (schema, components) = build_message(
        &ent_path,
        [build_log_time(now_plus_20ms), build_frame_nr(frame41)],
        [build_instances(nb_instances), build_rects(nb_instances)],
    );
    eprintln!("inserting into '{ent_path}':\nschema: {schema:#?}\ncomponents: {components:#?}");
    // eprintln!("---\ninserting into '{ent_path}': [log_time, frame_nr], [instances, rects]");
    store.insert(&schema, &components).unwrap();
    // eprintln!("{store}");

    let expected_instances = build_instances(nb_instances);
    let (schema, components) = build_message(
        &ent_path,
        [build_log_time(now), build_frame_nr(frame42)],
        [expected_instances.clone(), build_rects(nb_instances)],
    );
    eprintln!("inserting into '{ent_path}':\nschema: {schema:#?}\ncomponents: {components:#?}");
    // eprintln!("---\ninserting into '{ent_path}': [log_time, frame_nr], [instances]");
    store.insert(&schema, &components).unwrap();
    eprintln!("{store}");

    let expected_positions = build_positions(nb_instances);
    let (schema, components) = build_message(
        &ent_path,
        [build_log_time(now_minus_10ms), build_frame_nr(frame42)],
        [expected_positions.clone()],
    );
    eprintln!("inserting into '{ent_path}':\nschema: {schema:#?}\ncomponents: {components:#?}");
    // eprintln!("---\ninserting into '{ent_path}': [log_time, frame_nr], [positions]");
    store.insert(&schema, &components).unwrap();
    eprintln!("{store}");

    // TODO(cmc): push to a single timeline
    // TODO(cmc): pushing a component multiple times on the same timeline+time
    // TODO(cmc): query at 40, 41, 42, 43, 44

    let timeline = Timeline::new("frame_nr", TimeType::Sequence);
    let components = &["instances", "rects", "positions"];

    // Querying at a time where no data exists.
    let df = store
        .query(&timeline, &TimeQuery::LatestAt(40), &ent_path, components)
        .unwrap();
    eprintln!("{df}");

    // Querying a bunch of components that don't exist.
    let df = store
        .query(
            &timeline,
            &TimeQuery::LatestAt(40),
            &ent_path,
            &["they", "dont", "exist"],
        )
        .unwrap();
    eprintln!("{df}");

    let df = store
        .query(&timeline, &TimeQuery::LatestAt(44), &ent_path, components)
        .unwrap();
    eprintln!("{df}");

    use polars::prelude::Series;

    let instances = df.select_series(["instances"]).unwrap().pop().unwrap();
    let expected_instances = Series::try_from(("instances", expected_instances.1.boxed())).unwrap();
    assert_eq!(expected_instances, instances);

    let positions = df.select_series(["positions"]).unwrap().pop().unwrap();
    let expected_positions = Series::try_from(("positions", expected_positions.1.boxed())).unwrap();
    assert_eq!(expected_positions, positions);

    let rects = df.select_series(["rects"]).unwrap().pop().unwrap();
    let expected_rects = Series::try_from(("rects", expected_rects.1.boxed())).unwrap();
    assert_eq!(expected_rects, rects);
}
