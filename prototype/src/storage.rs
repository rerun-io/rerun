use crate::{IndexKey, IndexPathKey, TimeQuery, TypePath, TypePathComponent};

use std::collections::BTreeMap;
use std::sync::Arc;

use ahash::AHashMap;
use nohash_hasher::IntMap;

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
        type_path: TypePath,
        index_path: IndexPathKey,
        time: Time,
        value: T,
    ) -> Result<()> {
        if let Some(store) = self
            .data
            .entry(type_path)
            .or_insert_with(|| DataStoreTypeErased::new_individual::<T>())
            .write::<T>()
        {
            store.insert_individual(index_path, time, value)
        } else {
            Err(Error::WrongType)
        }
    }

    pub fn insert_batch<T: 'static>(
        &mut self,
        type_path: TypePath,
        index_path_prefix: IndexPathKey,
        time: Time,
        values: Batch<T>,
    ) -> Result<()> {
        if let Some(store) = self
            .data
            .entry(type_path)
            .or_insert_with(|| DataStoreTypeErased::new_batched::<T>())
            .write::<T>()
        {
            store.insert_batch(index_path_prefix, time, values)
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
        value: T,
    ) -> Result<()> {
        match self {
            Self::Individual(individual) => {
                individual.insert(index_path, time, value);
                Ok(())
            }
            Self::Batched(_) => Err(Error::BatchFollowedByIndividual),
        }
    }

    pub fn insert_batch(
        &mut self,
        index_path_prefix: IndexPathKey,
        time: Time,
        values: Batch<T>,
    ) -> Result<()> {
        match self {
            Self::Individual(_) => Err(Error::IndividualFollowedByBatch),
            Self::Batched(batched) => {
                batched.insert(index_path_prefix, time, values);
                Ok(())
            }
        }
    }
}

// ----------------------------------------------------------------------------

/// For a specific [`TypePath`].
pub struct IndividualDataHistory<Time, T> {
    /// fast to find latest value at a certain time.
    values: IntMap<IndexPathKey, BTreeMap<Time, T>>,
}

impl<Time: Ord, T> Default for IndividualDataHistory<Time, T> {
    fn default() -> Self {
        Self {
            values: Default::default(),
        }
    }
}

impl<Time: Ord, T> IndividualDataHistory<Time, T> {
    pub fn insert(&mut self, index_path: IndexPathKey, time: Time, value: T) {
        self.values
            .entry(index_path)
            .or_default()
            .insert(time, value);
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = (&IndexPathKey, &BTreeMap<Time, T>)> {
        self.values.iter()
    }
}

// ----------------------------------------------------------------------------

/// For a specific [`TypePath`].
pub struct BatchedDataHistory<Time, T> {
    /// The index is the path prefix (everything but the last value).
    batches_over_time: IntMap<IndexPathKey, BTreeMap<Time, Batch<T>>>,
}

impl<Time: Ord, T> Default for BatchedDataHistory<Time, T> {
    fn default() -> Self {
        Self {
            batches_over_time: Default::default(),
        }
    }
}

impl<Time: Ord, T> BatchedDataHistory<Time, T> {
    pub fn insert(&mut self, index_path_prefix: IndexPathKey, time: Time, values: Batch<T>) {
        self.batches_over_time
            .entry(index_path_prefix)
            .or_default()
            .insert(time, values);
    }

    pub fn iter(
        &self,
    ) -> impl ExactSizeIterator<Item = (&IndexPathKey, &BTreeMap<Time, Batch<T>>)> {
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
                latest_at(history.values.get(index_path)?, query_time).map(|(_time, value)| value)
            }
            Self::Batched(data) => {
                let (prefix, suffix) = index_path.clone().split_last();
                latest_at(data.batches_over_time.get(&prefix)?, query_time)?
                    .1
                    .get(&suffix)
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
                let everything_per_time = &batched.batches_over_time.get(index_path_prefix)?;
                let (_, map) = latest_at(everything_per_time, query_time)?;
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
                latest_at(history.values.get(&index_path)?, query_time).map(|(_time, value)| value)
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

pub fn visit_data<'s, Time: 'static + Ord, T: 'static>(
    store: &'s TypePathDataStore<Time>,
    time_query: &TimeQuery<Time>,
    primary_type_path: &TypePath,
    mut visit: impl FnMut(&'s T),
) -> Option<()> {
    let primary_data = store.get::<T>(primary_type_path)?;

    match primary_data {
        DataStore::Individual(primary) => {
            for (_index_path, values_over_time) in primary.iter() {
                query(values_over_time, time_query, |_time, primary| {
                    visit(primary);
                });
            }
        }
        DataStore::Batched(primary) => {
            for (_index_path_prefix, primary) in primary.iter() {
                query(primary, time_query, |_time, primary| {
                    for primary in primary.values() {
                        visit(primary);
                    }
                });
            }
        }
    }

    Some(())
}

pub fn visit_data_and_siblings<'s, Time: 'static + Clone + Ord, T: 'static, S1: 'static>(
    store: &'s TypePathDataStore<Time>,
    time_query: &TimeQuery<Time>,
    primary_type_path: &TypePath,
    (sibling1,): (&str,),
    mut visit: impl FnMut(&'s T, Option<&'s S1>),
) -> Option<()> {
    let primary_data = store.get::<T>(primary_type_path)?;
    let sibling1_path = sibling(primary_type_path, sibling1);

    match primary_data {
        DataStore::Individual(primary) => {
            let sibling1_reader = IndividualDataReader::<Time, S1>::new(store, &sibling1_path);
            for (index_path, values_over_time) in primary.iter() {
                query(values_over_time, time_query, |time, primary| {
                    let sibling1 = sibling1_reader.latest_at(index_path, time);
                    visit(primary, sibling1);
                });
            }
        }
        DataStore::Batched(primary) => {
            for (index_path_prefix, primary) in primary.iter() {
                let sibling1_store = store.get::<S1>(&sibling1_path);
                query(primary, time_query, |time, primary| {
                    let sibling1_reader =
                        BatchedDataReader::new(sibling1_store, index_path_prefix, time);
                    for (index_path_suffix, primary) in primary.iter() {
                        let sibling1 = sibling1_reader.latest_at(index_path_suffix);
                        visit(primary, sibling1);
                    }
                });
            }
        }
    }

    Some(())
}

fn sibling(type_path: &TypePath, name: &str) -> TypePath {
    let mut type_path = type_path.clone();
    type_path.pop_back();
    type_path.push_back(TypePathComponent::Name(name.into()));
    type_path
}
