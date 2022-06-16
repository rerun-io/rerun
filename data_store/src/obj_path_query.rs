//! Queries of the type "read these fields, from the object at this [`ObjPath`], over this time interval"

use log_types::{DataTrait, FieldName, IndexPath, IndexPathHash, LogId, ObjPath};

use crate::{storage::*, TimeQuery};

// ----------------------------------------------------------------------------

/// Do a time query for the single object the given path with one primary field.
pub fn visit_obj_data<'s, Time: 'static + Copy + Ord, T: DataTrait>(
    obj_store: &'s ObjStore<Time>,
    index_path: &IndexPath,
    field_name: &FieldName,
    time_query: &TimeQuery<Time>,
    mut visit: impl FnMut(&'s ObjPath, &'s LogId, &'s T),
) -> Option<()> {
    crate::profile_function!();

    if let Some(primary_data) = obj_store.get::<T>(field_name) {
        match primary_data {
            DataStore::Individual(primary) => {
                let index_path = IndexPathHash::from_path(index_path);
                if let Some(primary) = primary.values.get(&index_path) {
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
                let (index_path_prefix, index_path_suffix) =
                    index_path.clone().replace_last_with_placeholder();
                let index_path_prefix = IndexPathHash::from_path(&index_path_prefix);
                let index_path_suffix = index_path_suffix.hash();

                if let Some(primary) = primary.batches_over_time.get(&index_path_prefix) {
                    query(
                        &primary.history,
                        time_query,
                        |_time, (log_id, primary_batch)| {
                            if let Some(primary_value) = primary_batch.get(index_path_suffix) {
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
            DataStore::BatchSplat(_) => {
                tracing::error!("Used BatchSplat for a primary field {field_name:?}");
            }
        }
    }

    Some(())
}

/// Do a time query for the single object the given path with one primary field and one secondary field.
pub fn visit_obj_data_1<'s, Time: 'static + Copy + Ord, T: DataTrait, S1: DataTrait>(
    obj_store: &'s ObjStore<Time>,
    index_path: &IndexPath,
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
                let index_path = IndexPathHash::from_path(index_path);

                if let Some(primary) = primary.values.get(&index_path) {
                    let index_path_split = &primary.index_path_split;
                    let child1_reader = IndividualDataReader::<Time, S1>::new(obj_store, &child1);

                    query(
                        &primary.history,
                        time_query,
                        |time, (log_id, primary_value)| {
                            visit(
                                &primary.obj_path,
                                log_id,
                                primary_value,
                                child1_reader.latest_at(&index_path, index_path_split, time),
                            );
                        },
                    );
                }
            }
            DataStore::Batched(primary) => {
                let (index_path_prefix, index_path_suffix) =
                    index_path.clone().replace_last_with_placeholder();
                let index_path_prefix = IndexPathHash::from_path(&index_path_prefix);
                let index_path_suffix = index_path_suffix.hash();

                if let Some(primary) = primary.batches_over_time.get(&index_path_prefix) {
                    let child1_store = obj_store.get::<S1>(&child1);

                    query(
                        &primary.history,
                        time_query,
                        |time, (log_id, primary_batch)| {
                            if let Some(primary_value) = primary_batch.get(index_path_suffix) {
                                let child1_reader =
                                    BatchedDataReader::new(child1_store, &index_path_prefix, time);

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
            DataStore::BatchSplat(_) => {
                tracing::error!("Used BatchSplat for a primary field {field_name:?}");
            }
        }
    }

    Some(())
}

/// Do a time query for the single object the given path with one primary field and two secondary field.
pub fn visit_obj_data_2<
    's,
    Time: 'static + Copy + Ord,
    T: DataTrait,
    S1: DataTrait,
    S2: DataTrait,
>(
    obj_store: &'s ObjStore<Time>,
    index_path: &IndexPath,
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
                let index_path = IndexPathHash::from_path(index_path);

                if let Some(primary) = primary.values.get(&index_path) {
                    let index_path_split = &primary.index_path_split;
                    let child1_reader = IndividualDataReader::<Time, S1>::new(obj_store, &child1);
                    let child2_reader = IndividualDataReader::<Time, S2>::new(obj_store, &child2);

                    query(
                        &primary.history,
                        time_query,
                        |time, (log_id, primary_value)| {
                            visit(
                                &primary.obj_path,
                                log_id,
                                primary_value,
                                child1_reader.latest_at(&index_path, index_path_split, time),
                                child2_reader.latest_at(&index_path, index_path_split, time),
                            );
                        },
                    );
                }
            }
            DataStore::Batched(primary) => {
                let (index_path_prefix, index_path_suffix) =
                    index_path.clone().replace_last_with_placeholder();
                let index_path_prefix = IndexPathHash::from_path(&index_path_prefix);
                let index_path_suffix = index_path_suffix.hash();

                if let Some(primary) = primary.batches_over_time.get(&index_path_prefix) {
                    let child1_store = obj_store.get::<S1>(&child1);
                    let child2_store = obj_store.get::<S2>(&child2);

                    query(
                        &primary.history,
                        time_query,
                        |time, (log_id, primary_batch)| {
                            if let Some(primary_value) = primary_batch.get(index_path_suffix) {
                                let child1_reader =
                                    BatchedDataReader::new(child1_store, &index_path_prefix, time);
                                let child2_reader =
                                    BatchedDataReader::new(child2_store, &index_path_prefix, time);

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
            DataStore::BatchSplat(_) => {
                tracing::error!("Used BatchSplat for a primary field {field_name:?}");
            }
        }
    }

    Some(())
}

/// Do a time query for the single object the given path with one primary field and three secondary field.
pub fn visit_obj_data_3<
    's,
    Time: 'static + Copy + Ord,
    T: DataTrait,
    S1: DataTrait,
    S2: DataTrait,
    S3: DataTrait,
>(
    obj_store: &'s ObjStore<Time>,
    index_path: &IndexPath,
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
                let index_path = IndexPathHash::from_path(index_path);

                if let Some(primary) = primary.values.get(&index_path) {
                    let index_path_split = &primary.index_path_split;
                    let child1_reader = IndividualDataReader::<Time, S1>::new(obj_store, &child1);
                    let child2_reader = IndividualDataReader::<Time, S2>::new(obj_store, &child2);
                    let child3_reader = IndividualDataReader::<Time, S3>::new(obj_store, &child3);

                    query(
                        &primary.history,
                        time_query,
                        |time, (log_id, primary_value)| {
                            visit(
                                &primary.obj_path,
                                log_id,
                                primary_value,
                                child1_reader.latest_at(&index_path, index_path_split, time),
                                child2_reader.latest_at(&index_path, index_path_split, time),
                                child3_reader.latest_at(&index_path, index_path_split, time),
                            );
                        },
                    );
                }
            }
            DataStore::Batched(primary) => {
                let (index_path_prefix, index_path_suffix) =
                    index_path.clone().replace_last_with_placeholder();
                let index_path_prefix = IndexPathHash::from_path(&index_path_prefix);
                let index_path_suffix = index_path_suffix.hash();

                if let Some(primary) = primary.batches_over_time.get(&index_path_prefix) {
                    let child1_store = obj_store.get::<S1>(&child1);
                    let child2_store = obj_store.get::<S2>(&child2);
                    let child3_store = obj_store.get::<S3>(&child3);

                    query(
                        &primary.history,
                        time_query,
                        |time, (log_id, primary_batch)| {
                            if let Some(primary_value) = primary_batch.get(index_path_suffix) {
                                let child1_reader =
                                    BatchedDataReader::new(child1_store, &index_path_prefix, time);
                                let child2_reader =
                                    BatchedDataReader::new(child2_store, &index_path_prefix, time);
                                let child3_reader =
                                    BatchedDataReader::new(child3_store, &index_path_prefix, time);

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
            DataStore::BatchSplat(_) => {
                tracing::error!("Used BatchSplat for a primary field {field_name:?}");
            }
        }
    }

    Some(())
}
