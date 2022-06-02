use std::collections::BTreeMap;
use std::sync::Arc;

use ahash::AHashMap;
use nohash_hasher::IntMap;

use log_types::{DataPath, LogId};

use crate::{IndexKey, IndexPathKey, TimeQuery, TypePath, TypePathComponent};

#[derive(Clone, Copy, Debug)]
pub enum Error {
    /// First stored as a batch, then individually. Not supported.
    BatchFollowedByIndividual,

    /// First stored individually, then followed by a batch. Not supported.
    IndividualFollowedByBatch,

    /// One type was first logged, then another.
    WrongType,
}

pub type Result<T> = std::result::Result<T, Error>;

// ----------------------------------------------------------------------------

/// Can be shared between different timelines.
pub type Batch<T> = Arc<IntMap<IndexKey, T>>;

/// One per each time source.
pub struct TypePathDataStore<Time> {
    data: AHashMap<TypePath, DataStoreTypeErased<Time>>,
}

impl<Time> Default for TypePathDataStore<Time> {
    fn default() -> Self {
        Self {
            data: Default::default(),
        }
    }
}

impl<Time: 'static + Ord> TypePathDataStore<Time> {
    pub fn insert_individual<T: 'static>(
        &mut self,
        data_path: DataPath,
        time: Time,
        log_id: LogId,
        value: T,
    ) -> Result<()> {
        let parent_object_path = data_path.parent();
        let (type_path, index_path) = crate::into_type_path(data_path);

        if let Some(store) = self
            .data
            .entry(type_path)
            .or_insert_with(|| DataStoreTypeErased::new_individual::<T>())
            .write::<T>()
        {
            store.insert_individual(index_path, time, parent_object_path, log_id, value)
        } else {
            Err(Error::WrongType)
        }
    }

    pub fn insert_batch<T: 'static>(
        &mut self,
        type_path: TypePath,
        index_path_prefix: IndexPathKey,
        time: Time,
        log_id: LogId,
        batch: Batch<T>,
    ) -> Result<()> {
        let parent_object_path = crate::batch_parent_object_path(&type_path, &index_path_prefix);

        if let Some(store) = self
            .data
            .entry(type_path)
            .or_insert_with(|| DataStoreTypeErased::new_batched::<T>())
            .write::<T>()
        {
            store.insert_batch(index_path_prefix, time, parent_object_path, log_id, batch)
        } else {
            Err(Error::WrongType)
        }
    }

    pub fn get<T: 'static>(&self, type_path: &TypePath) -> Option<&DataStore<Time, T>> {
        self.data.get(type_path).and_then(|x| x.read::<T>())
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = (&TypePath, &DataStoreTypeErased<Time>)> {
        self.data.iter()
    }
}

// ----------------------------------------------------------------------------

/// Type-erased version of [`DataStore`].
pub struct DataStoreTypeErased<Time>(Box<dyn std::any::Any>, std::marker::PhantomData<Time>);

impl<Time: 'static + Ord> DataStoreTypeErased<Time> {
    fn new_individual<T: 'static>() -> Self {
        Self(
            Box::new(DataStore::<Time, T>::new_individual()),
            Default::default(),
        )
    }

    fn new_batched<T: 'static>() -> Self {
        Self(
            Box::new(DataStore::<Time, T>::new_batched()),
            Default::default(),
        )
    }

    pub fn is<T: 'static>(&self) -> bool {
        self.0.is::<DataStore<Time, T>>()
    }

    pub fn read_no_warn<T: 'static>(&self) -> Option<&DataStore<Time, T>> {
        self.0.downcast_ref::<DataStore<Time, T>>()
    }

    pub fn read<T: 'static>(&self) -> Option<&DataStore<Time, T>> {
        if let Some(read) = self.read_no_warn() {
            Some(read)
        } else {
            tracing::warn!("Expected {}", std::any::type_name::<T>());
            None
        }
    }

    pub fn write<T: 'static>(&mut self) -> Option<&mut DataStore<Time, T>> {
        self.0.downcast_mut::<DataStore<Time, T>>()
    }
}

// ----------------------------------------------------------------------------

pub enum DataStore<Time, T> {
    /// Individual data at this path.
    Individual(IndividualDataHistory<Time, T>),

    Batched(BatchedDataHistory<Time, T>),
}

impl<Time: Ord, T: 'static> DataStore<Time, T> {
    fn new_individual() -> Self {
        Self::Individual(Default::default())
    }

    fn new_batched() -> Self {
        Self::Batched(Default::default())
    }

    pub fn insert_individual(
        &mut self,
        index_path: IndexPathKey,
        time: Time,
        parent_object_path: DataPath,
        log_id: LogId,
        value: T,
    ) -> Result<()> {
        match self {
            Self::Individual(individual) => {
                individual.insert(index_path, time, parent_object_path, log_id, value);
                Ok(())
            }
            Self::Batched(_) => Err(Error::BatchFollowedByIndividual),
        }
    }

    pub fn insert_batch(
        &mut self,
        index_path_prefix: IndexPathKey,
        time: Time,
        parent_object_path: DataPath,
        log_id: LogId,
        batch: Batch<T>,
    ) -> Result<()> {
        match self {
            Self::Individual(_) => Err(Error::IndividualFollowedByBatch),
            Self::Batched(batched) => {
                batched.insert(index_path_prefix, time, parent_object_path, log_id, batch);
                Ok(())
            }
        }
    }
}

// ----------------------------------------------------------------------------

/// For a specific [`TypePath`].
pub struct IndividualDataHistory<Time, T> {
    /// fast to find latest value at a certain time.
    values: IntMap<IndexPathKey, IndividualHistory<Time, T>>,
}

pub struct IndividualHistory<Time, T> {
    /// Path to the parent object.
    ///
    /// This is so that we can quickly check for object visibility when rendering.
    pub parent_object_path: DataPath,
    pub history: BTreeMap<Time, (LogId, T)>,
}

impl<Time: Ord, T> Default for IndividualDataHistory<Time, T> {
    fn default() -> Self {
        Self {
            values: Default::default(),
        }
    }
}

impl<Time: Ord, T> IndividualDataHistory<Time, T> {
    pub fn insert(
        &mut self,
        index_path: IndexPathKey,
        time: Time,
        parent_object_path: DataPath,
        log_id: LogId,
        value: T,
    ) {
        self.values
            .entry(index_path)
            .or_insert_with(|| IndividualHistory {
                parent_object_path,
                history: Default::default(),
            })
            .history
            .insert(time, (log_id, value));
    }

    pub fn iter(
        &self,
    ) -> impl ExactSizeIterator<Item = (&IndexPathKey, &IndividualHistory<Time, T>)> {
        self.values.iter()
    }
}

// ----------------------------------------------------------------------------

/// For a specific [`TypePath`].
pub struct BatchedDataHistory<Time, T> {
    /// The index is the path prefix (everything but the last value).
    batches_over_time: IntMap<IndexPathKey, BatchHistory<Time, T>>,
}

pub struct BatchHistory<Time, T> {
    /// Path to the parent object, owning the batch.
    ///
    /// For instance, this is the path to the whole point cloud.
    ///
    /// This is so that we can quickly check for object visibility when rendering.
    pub parent_object_path: DataPath,

    pub history: BTreeMap<Time, (LogId, Batch<T>)>,
}

impl<Time: Ord, T> Default for BatchedDataHistory<Time, T> {
    fn default() -> Self {
        Self {
            batches_over_time: Default::default(),
        }
    }
}

impl<Time: Ord, T> BatchedDataHistory<Time, T> {
    pub fn insert(
        &mut self,
        index_path_prefix: IndexPathKey,
        time: Time,
        parent_object_path: DataPath,
        log_id: LogId,
        batch: Batch<T>,
    ) {
        self.batches_over_time
            .entry(index_path_prefix)
            .or_insert_with(|| BatchHistory {
                parent_object_path,
                history: Default::default(),
            })
            .history
            .insert(time, (log_id, batch));
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = (&IndexPathKey, &BatchHistory<Time, T>)> {
        self.batches_over_time.iter()
    }
}

// ----------------------------------------------------------------------------

pub enum IndividualDataReader<'store, Time, T> {
    None,
    Individual(&'store IndividualDataHistory<Time, T>),
    Batched(&'store BatchedDataHistory<Time, T>),
}

impl<'store, Time: 'static + Ord, T: 'static> IndividualDataReader<'store, Time, T> {
    pub fn new(store: &'store TypePathDataStore<Time>, type_path: &TypePath) -> Self {
        if let Some(data) = store.get::<T>(type_path) {
            match data {
                DataStore::Individual(individual) => Self::Individual(individual),
                DataStore::Batched(batched) => Self::Batched(batched),
            }
        } else {
            Self::None
        }
    }

    pub fn latest_at(&self, index_path: &IndexPathKey, query_time: &Time) -> Option<&'store T> {
        match self {
            Self::None => None,
            Self::Individual(history) => {
                latest_at(&history.values.get(index_path)?.history, query_time)
                    .map(|(_time, (_log_id, value))| value)
            }
            Self::Batched(data) => {
                let (prefix, suffix) = index_path.clone().split_last();
                let (_time, (_log_id, batch)) =
                    latest_at(&data.batches_over_time.get(&prefix)?.history, query_time)?;
                batch.get(&suffix)
            }
        }
    }
}

// ----------------------------------------------------------------------------

pub enum BatchedDataReader<'store, Time, T> {
    None,
    Individual(IndexPathKey, Time, &'store IndividualDataHistory<Time, T>),
    Batched(&'store IntMap<IndexKey, T>),
}

impl<'store, Time: Clone + Ord, T: 'static> BatchedDataReader<'store, Time, T> {
    pub fn new(
        data: Option<&'store DataStore<Time, T>>,
        index_path_prefix: &IndexPathKey,
        query_time: &Time,
    ) -> Self {
        data.and_then(|data| Self::new_opt(data, index_path_prefix, query_time))
            .unwrap_or(Self::None)
    }

    fn new_opt(
        data: &'store DataStore<Time, T>,
        index_path_prefix: &IndexPathKey,
        query_time: &Time,
    ) -> Option<Self> {
        match data {
            DataStore::Individual(individual) => Some(Self::Individual(
                index_path_prefix.clone(),
                query_time.clone(),
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
                index_path.push_back(index_path_suffix.clone());
                latest_at(&history.values.get(&index_path)?.history, query_time)
                    .map(|(_time, (_log_id, value))| value)
            }
            Self::Batched(data) => data.get(index_path_suffix),
        }
    }
}

// ----------------------------------------------------------------------------

fn latest_at<'data, Time: Ord, T>(
    data_over_time: &'data BTreeMap<Time, T>,
    query_time: &'_ Time,
) -> Option<(&'data Time, &'data T)> {
    data_over_time.range(..=query_time).rev().next()
}

fn values_in_range<'data, Time: Ord, T>(
    data_over_time: &'data BTreeMap<Time, T>,
    time_range: &'_ std::ops::RangeInclusive<Time>,
) -> impl Iterator<Item = (&'data Time, &'data T)> {
    data_over_time.range(time_range.start()..=time_range.end())
}

pub fn query<'data, Time: Ord, T>(
    data_over_time: &'data BTreeMap<Time, T>,
    time_query: &TimeQuery<Time>,
    mut visit: impl FnMut(&Time, &'data T),
) {
    match time_query {
        TimeQuery::LatestAt(query_time) => {
            if let Some((_data_time, data)) = latest_at(data_over_time, query_time) {
                // we use `query_time` here instead of a`data_time`
                // because we want to also query for the latest radius, not the latest radius at the time of the position.
                visit(query_time, data);
            }
        }
        TimeQuery::Range(query_range) => {
            for (data_time, data) in values_in_range(data_over_time, query_range) {
                visit(data_time, data);
            }
        }
    }
}

// ----------------------------------------------------------------------------

/// The visitor is called with the object data path, the closest individually addressable parent object. It can be used to test if the object should be visible.
pub fn visit_data<'s, Time: 'static + Ord, T: 'static>(
    time_query: &TimeQuery<Time>,
    primary_data: &'s DataStore<Time, T>,
    mut visit: impl FnMut(&'s DataPath, &'s LogId, &'s T),
) -> Option<()> {
    crate::profile_function!();

    match primary_data {
        DataStore::Individual(primary) => {
            for (_index_path, primary) in primary.iter() {
                query(
                    &primary.history,
                    time_query,
                    |_time, (log_id, primary_value)| {
                        visit(&primary.parent_object_path, log_id, primary_value);
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
                        for (_index_path_suffix, primary_value) in primary_batch.iter() {
                            visit(&primary.parent_object_path, log_id, primary_value);
                        }
                    },
                );
            }
        }
    }

    Some(())
}

pub fn visit_data_and_1_sibling<'s, Time: 'static + Clone + Ord, T: 'static, S1: 'static>(
    store: &'s TypePathDataStore<Time>,
    time_query: &TimeQuery<Time>,
    primary_type_path: &TypePath,
    primary_data: &'s DataStore<Time, T>,
    (sibling1,): (&str,),
    mut visit: impl FnMut(&'s DataPath, &'s LogId, &'s T, Option<&'s S1>),
) -> Option<()> {
    crate::profile_function!();

    let sibling1_path = sibling(primary_type_path, sibling1);

    match primary_data {
        DataStore::Individual(primary) => {
            let sibling1_reader = IndividualDataReader::<Time, S1>::new(store, &sibling1_path);

            for (index_path, primary) in primary.iter() {
                query(
                    &primary.history,
                    time_query,
                    |time, (log_id, primary_value)| {
                        visit(
                            &primary.parent_object_path,
                            log_id,
                            primary_value,
                            sibling1_reader.latest_at(index_path, time),
                        );
                    },
                );
            }
        }
        DataStore::Batched(primary) => {
            for (index_path_prefix, primary) in primary.iter() {
                let sibling1_store = store.get::<S1>(&sibling1_path);

                query(
                    &primary.history,
                    time_query,
                    |time, (log_id, primary_batch)| {
                        let sibling1_reader =
                            BatchedDataReader::new(sibling1_store, index_path_prefix, time);

                        for (index_path_suffix, primary_value) in primary_batch.iter() {
                            visit(
                                &primary.parent_object_path,
                                log_id,
                                primary_value,
                                sibling1_reader.latest_at(index_path_suffix),
                            );
                        }
                    },
                );
            }
        }
    }

    Some(())
}

pub fn visit_data_and_2_siblings<
    's,
    Time: 'static + Clone + Ord,
    T: 'static,
    S1: 'static,
    S2: 'static,
>(
    store: &'s TypePathDataStore<Time>,
    time_query: &TimeQuery<Time>,
    primary_type_path: &TypePath,
    primary_data: &'s DataStore<Time, T>,
    (sibling1, sibling2): (&str, &str),
    mut visit: impl FnMut(&'s DataPath, &'s LogId, &'s T, Option<&'s S1>, Option<&'s S2>),
) -> Option<()> {
    crate::profile_function!();

    let sibling1_path = sibling(primary_type_path, sibling1);
    let sibling2_path = sibling(primary_type_path, sibling2);

    match primary_data {
        DataStore::Individual(primary) => {
            let sibling1_reader = IndividualDataReader::<Time, S1>::new(store, &sibling1_path);
            let sibling2_reader = IndividualDataReader::<Time, S2>::new(store, &sibling2_path);

            for (index_path, primary) in primary.iter() {
                query(
                    &primary.history,
                    time_query,
                    |time, (log_id, primary_value)| {
                        visit(
                            &primary.parent_object_path,
                            log_id,
                            primary_value,
                            sibling1_reader.latest_at(index_path, time),
                            sibling2_reader.latest_at(index_path, time),
                        );
                    },
                );
            }
        }
        DataStore::Batched(primary) => {
            for (index_path_prefix, primary) in primary.iter() {
                let sibling1_store = store.get::<S1>(&sibling1_path);
                let sibling2_store = store.get::<S2>(&sibling2_path);

                query(
                    &primary.history,
                    time_query,
                    |time, (log_id, primary_batch)| {
                        let sibling1_reader =
                            BatchedDataReader::new(sibling1_store, index_path_prefix, time);
                        let sibling2_reader =
                            BatchedDataReader::new(sibling2_store, index_path_prefix, time);

                        for (index_path_suffix, primary_value) in primary_batch.iter() {
                            visit(
                                &primary.parent_object_path,
                                log_id,
                                primary_value,
                                sibling1_reader.latest_at(index_path_suffix),
                                sibling2_reader.latest_at(index_path_suffix),
                            );
                        }
                    },
                );
            }
        }
    }

    Some(())
}

pub fn visit_data_and_3_siblings<
    's,
    Time: 'static + Clone + Ord,
    T: 'static,
    S1: 'static,
    S2: 'static,
    S3: 'static,
>(
    store: &'s TypePathDataStore<Time>,
    time_query: &TimeQuery<Time>,
    primary_type_path: &TypePath,
    primary_data: &'s DataStore<Time, T>,
    (sibling1, sibling2, sibling3): (&str, &str, &str),
    mut visit: impl FnMut(
        &'s DataPath,
        &'s LogId,
        &'s T,
        Option<&'s S1>,
        Option<&'s S2>,
        Option<&'s S3>,
    ),
) -> Option<()> {
    crate::profile_function!();

    let sibling1_path = sibling(primary_type_path, sibling1);
    let sibling2_path = sibling(primary_type_path, sibling2);
    let sibling3_path = sibling(primary_type_path, sibling3);

    match primary_data {
        DataStore::Individual(primary) => {
            let sibling1_reader = IndividualDataReader::<Time, S1>::new(store, &sibling1_path);
            let sibling2_reader = IndividualDataReader::<Time, S2>::new(store, &sibling2_path);
            let sibling3_reader = IndividualDataReader::<Time, S3>::new(store, &sibling3_path);

            for (index_path, primary) in primary.iter() {
                query(
                    &primary.history,
                    time_query,
                    |time, (log_id, primary_value)| {
                        visit(
                            &primary.parent_object_path,
                            log_id,
                            primary_value,
                            sibling1_reader.latest_at(index_path, time),
                            sibling2_reader.latest_at(index_path, time),
                            sibling3_reader.latest_at(index_path, time),
                        );
                    },
                );
            }
        }
        DataStore::Batched(primary) => {
            for (index_path_prefix, primary) in primary.iter() {
                let sibling1_store = store.get::<S1>(&sibling1_path);
                let sibling2_store = store.get::<S2>(&sibling2_path);
                let sibling3_store = store.get::<S3>(&sibling3_path);

                query(
                    &primary.history,
                    time_query,
                    |time, (log_id, primary_batch)| {
                        let sibling1_reader =
                            BatchedDataReader::new(sibling1_store, index_path_prefix, time);
                        let sibling2_reader =
                            BatchedDataReader::new(sibling2_store, index_path_prefix, time);
                        let sibling3_reader =
                            BatchedDataReader::new(sibling3_store, index_path_prefix, time);

                        for (index_path_suffix, primary_value) in primary_batch.iter() {
                            visit(
                                &primary.parent_object_path,
                                log_id,
                                primary_value,
                                sibling1_reader.latest_at(index_path_suffix),
                                sibling2_reader.latest_at(index_path_suffix),
                                sibling3_reader.latest_at(index_path_suffix),
                            );
                        }
                    },
                );
            }
        }
    }

    Some(())
}

fn sibling(type_path: &TypePath, name: &str) -> TypePath {
    let mut type_path = type_path.clone();
    type_path.pop_back();
    type_path.push_back(TypePathComponent::String(name.into()));
    type_path
}
