//! Queries of the type "read these fields, from all objects of this [`ObjTypePath`], over this time interval"

use log_types::{DataTrait, FieldName, LogId, ObjPath};

use crate::{storage::*, TimeQuery};

// ----------------------------------------------------------------------------

/// Query all objects of the same type path (but different index paths).
pub fn visit_type_data<'s, Time: 'static + Copy + Ord, T: DataTrait>(
    obj_store: &'s ObjStore<Time>,
    field_name: &FieldName,
    time_query: &TimeQuery<Time>,
    mut visit: impl FnMut(&'s ObjPath, &'s LogId, &'s T),
) -> Option<()> {
    crate::profile_function!();

    if let Some(primary_data) = obj_store.get::<T>(field_name) {
        match primary_data {
            DataStore::Individual(primary) => {
                for (_index_path, primary) in primary.iter() {
                    query(
                        &primary.history,
                        time_query,
                        |_time, (log_id, primary_value)| {
                            visit(&primary.obj_path, log_id, primary_value);
                        },
                    );
                }
            }
            DataStore::Batched(primary) => {
                for (_index_path_prefix, primary) in primary.iter() {
                    query(
                        &primary.history,
                        time_query,
                        |_time, (log_id, primary_batch)| {
                            for (index_path_suffix, primary_value) in primary_batch.iter() {
                                visit(
                                    obj_store.obj_path_or_die(index_path_suffix),
                                    log_id,
                                    primary_value,
                                );
                            }
                        },
                    );
                }
            }
        }
    }

    Some(())
}

pub fn visit_type_data_1<'s, Time: 'static + Copy + Ord, T: DataTrait, S1: DataTrait>(
    obj_store: &'s ObjStore<Time>,
    field_name: &FieldName,
    time_query: &TimeQuery<Time>,
    (child1,): (&str,),
    mut visit: impl FnMut(&'s ObjPath, &'s LogId, &'s T, Option<&'s S1>),
) -> Option<()> {
    crate::profile_function!();

    if let Some(primary_data) = obj_store.get::<T>(field_name) {
        let child1 = FieldName::from(child1);

        match primary_data {
            DataStore::Individual(primary) => {
                let child1_reader = IndividualDataReader::<Time, S1>::new(obj_store, &child1);

                for (index_path, primary) in primary.iter() {
                    query(
                        &primary.history,
                        time_query,
                        |time, (log_id, primary_value)| {
                            visit(
                                &primary.obj_path,
                                log_id,
                                primary_value,
                                child1_reader.latest_at(index_path, time),
                            );
                        },
                    );
                }
            }
            DataStore::Batched(primary) => {
                for (index_path_prefix, primary) in primary.iter() {
                    let child1_store = obj_store.get::<S1>(&child1);

                    query(
                        &primary.history,
                        time_query,
                        |time, (log_id, primary_batch)| {
                            let child1_reader =
                                BatchedDataReader::new(child1_store, index_path_prefix, time);

                            for (index_path_suffix, primary_value) in primary_batch.iter() {
                                visit(
                                    obj_store.obj_path_or_die(index_path_suffix),
                                    log_id,
                                    primary_value,
                                    child1_reader.latest_at(index_path_suffix),
                                );
                            }
                        },
                    );
                }
            }
        }
    }

    Some(())
}

pub fn visit_type_data_2<
    's,
    Time: 'static + Copy + Ord,
    T: DataTrait,
    S1: DataTrait,
    S2: DataTrait,
>(
    obj_store: &'s ObjStore<Time>,
    field_name: &FieldName,
    time_query: &TimeQuery<Time>,
    (child1, child2): (&str, &str),
    mut visit: impl FnMut(&'s ObjPath, &'s LogId, &'s T, Option<&'s S1>, Option<&'s S2>),
) -> Option<()> {
    crate::profile_function!();

    if let Some(primary_data) = obj_store.get::<T>(field_name) {
        let child1 = FieldName::from(child1);
        let child2 = FieldName::from(child2);

        match primary_data {
            DataStore::Individual(primary) => {
                let child1_reader = IndividualDataReader::<Time, S1>::new(obj_store, &child1);
                let child2_reader = IndividualDataReader::<Time, S2>::new(obj_store, &child2);

                for (index_path, primary) in primary.iter() {
                    query(
                        &primary.history,
                        time_query,
                        |time, (log_id, primary_value)| {
                            visit(
                                &primary.obj_path,
                                log_id,
                                primary_value,
                                child1_reader.latest_at(index_path, time),
                                child2_reader.latest_at(index_path, time),
                            );
                        },
                    );
                }
            }
            DataStore::Batched(primary) => {
                for (index_path_prefix, primary) in primary.iter() {
                    let child1_store = obj_store.get::<S1>(&child1);
                    let child2_store = obj_store.get::<S2>(&child2);

                    query(
                        &primary.history,
                        time_query,
                        |time, (log_id, primary_batch)| {
                            let child1_reader =
                                BatchedDataReader::new(child1_store, index_path_prefix, time);
                            let child2_reader =
                                BatchedDataReader::new(child2_store, index_path_prefix, time);

                            for (index_path_suffix, primary_value) in primary_batch.iter() {
                                visit(
                                    obj_store.obj_path_or_die(index_path_suffix),
                                    log_id,
                                    primary_value,
                                    child1_reader.latest_at(index_path_suffix),
                                    child2_reader.latest_at(index_path_suffix),
                                );
                            }
                        },
                    );
                }
            }
        }
    }

    Some(())
}

pub fn visit_type_data_3<
    's,
    Time: 'static + Copy + Ord,
    T: DataTrait,
    S1: DataTrait,
    S2: DataTrait,
    S3: DataTrait,
>(
    obj_store: &'s ObjStore<Time>,
    field_name: &FieldName,
    time_query: &TimeQuery<Time>,
    (child1, child2, child3): (&str, &str, &str),
    mut visit: impl FnMut(&'s ObjPath, &'s LogId, &'s T, Option<&'s S1>, Option<&'s S2>, Option<&'s S3>),
) -> Option<()> {
    crate::profile_function!();

    if let Some(primary_data) = obj_store.get::<T>(field_name) {
        let child1 = FieldName::from(child1);
        let child2 = FieldName::from(child2);
        let child3 = FieldName::from(child3);

        match primary_data {
            DataStore::Individual(primary) => {
                let child1_reader = IndividualDataReader::<Time, S1>::new(obj_store, &child1);
                let child2_reader = IndividualDataReader::<Time, S2>::new(obj_store, &child2);
                let child3_reader = IndividualDataReader::<Time, S3>::new(obj_store, &child3);

                for (index_path, primary) in primary.iter() {
                    query(
                        &primary.history,
                        time_query,
                        |time, (log_id, primary_value)| {
                            visit(
                                &primary.obj_path,
                                log_id,
                                primary_value,
                                child1_reader.latest_at(index_path, time),
                                child2_reader.latest_at(index_path, time),
                                child3_reader.latest_at(index_path, time),
                            );
                        },
                    );
                }
            }
            DataStore::Batched(primary) => {
                for (index_path_prefix, primary) in primary.iter() {
                    let child1_store = obj_store.get::<S1>(&child1);
                    let child2_store = obj_store.get::<S2>(&child2);
                    let child3_store = obj_store.get::<S3>(&child3);

                    query(
                        &primary.history,
                        time_query,
                        |time, (log_id, primary_batch)| {
                            let child1_reader =
                                BatchedDataReader::new(child1_store, index_path_prefix, time);
                            let child2_reader =
                                BatchedDataReader::new(child2_store, index_path_prefix, time);
                            let child3_reader =
                                BatchedDataReader::new(child3_store, index_path_prefix, time);

                            for (index_path_suffix, primary_value) in primary_batch.iter() {
                                visit(
                                    obj_store.obj_path_or_die(index_path_suffix),
                                    log_id,
                                    primary_value,
                                    child1_reader.latest_at(index_path_suffix),
                                    child2_reader.latest_at(index_path_suffix),
                                    child3_reader.latest_at(index_path_suffix),
                                );
                            }
                        },
                    );
                }
            }
        }
    }

    Some(())
}
