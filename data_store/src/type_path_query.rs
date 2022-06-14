use nohash_hasher::IntMap;

use log_types::{DataTrait, FieldName, IndexKey, IndexPath, LogId, ObjPath};

use crate::{storage::*, TimeQuery};

// ----------------------------------------------------------------------------

enum IndividualDataReader<'store, Time, T> {
    None,
    Individual(&'store IndividualDataHistory<Time, T>),
    Batched(&'store BatchedDataHistory<Time, T>),
}

impl<'store, Time: 'static + Copy + Ord, T: DataTrait> IndividualDataReader<'store, Time, T> {
    pub fn new(store: &'store ObjStore<Time>, field_name: &FieldName) -> Self {
        if let Some(data) = store.get::<T>(field_name) {
            match data {
                DataStore::Individual(individual) => Self::Individual(individual),
                DataStore::Batched(batched) => Self::Batched(batched),
            }
        } else {
            Self::None
        }
    }

    pub fn latest_at(&self, index_path: &IndexPath, query_time: &Time) -> Option<&'store T> {
        match self {
            Self::None => None,
            Self::Individual(history) => {
                latest_at(&history.values.get(index_path)?.history, query_time)
                    .map(|(_time, (_log_id, value))| value)
            }
            Self::Batched(data) => {
                let (prefix, suffix) = index_path.clone().replace_last_with_placeholder();
                let (_time, (_log_id, batch)) =
                    latest_at(&data.batches_over_time.get(&prefix)?.history, query_time)?;
                batch.get(&suffix)
            }
        }
    }
}

// ----------------------------------------------------------------------------

enum BatchedDataReader<'store, Time, T> {
    None,
    Individual(IndexPath, Time, &'store IndividualDataHistory<Time, T>),
    Batched(&'store IntMap<IndexKey, T>),
}

impl<'store, Time: Copy + Ord, T: DataTrait> BatchedDataReader<'store, Time, T> {
    pub fn new(
        data: Option<&'store DataStore<Time, T>>,
        index_path_prefix: &IndexPath,
        query_time: &Time,
    ) -> Self {
        data.and_then(|data| Self::new_opt(data, index_path_prefix, query_time))
            .unwrap_or(Self::None)
    }

    fn new_opt(
        data: &'store DataStore<Time, T>,
        index_path_prefix: &IndexPath,
        query_time: &Time,
    ) -> Option<Self> {
        match data {
            DataStore::Individual(individual) => Some(Self::Individual(
                index_path_prefix.clone(),
                *query_time,
                individual,
            )),
            DataStore::Batched(batched) => {
                let everything_per_time =
                    &batched.batches_over_time.get(index_path_prefix)?.history;
                let (_time, (_log_id, map)) = latest_at(everything_per_time, query_time)?;
                Some(Self::Batched(map))
            }
        }
    }

    pub fn latest_at(&self, index_path_suffix: &IndexKey) -> Option<&'store T> {
        match self {
            Self::None => None,
            Self::Individual(index_path_prefix, query_time, history) => {
                let mut index_path = index_path_prefix.clone();
                index_path.replace_last_placeholder_with(index_path_suffix.clone());
                latest_at(&history.values.get(&index_path)?.history, query_time)
                    .map(|(_time, (_log_id, value))| value)
            }
            Self::Batched(data) => data.get(index_path_suffix),
        }
    }
}

// ----------------------------------------------------------------------------

/// The visitor is called with the object data path, the closest individually addressable parent object. It can be used to test if the object should be visible.
pub fn visit_data<'s, Time: 'static + Copy + Ord, T: DataTrait>(
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
                                    obj_store
                                        .obj_paths_from_batch_suffix
                                        .get(index_path_suffix)
                                        .unwrap(),
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

pub fn visit_data_and_1_child<'s, Time: 'static + Copy + Ord, T: DataTrait, S1: DataTrait>(
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
                                    obj_store
                                        .obj_paths_from_batch_suffix
                                        .get(index_path_suffix)
                                        .unwrap(),
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

pub fn visit_data_and_2_children<
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
                                    obj_store
                                        .obj_paths_from_batch_suffix
                                        .get(index_path_suffix)
                                        .unwrap(),
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

pub fn visit_data_and_3_children<
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
                                    obj_store
                                        .obj_paths_from_batch_suffix
                                        .get(index_path_suffix)
                                        .unwrap(),
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
