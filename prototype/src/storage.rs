use crate::*;

use std::collections::BTreeMap;
use std::sync::Arc;

use ahash::AHashMap;
use nohash_hasher::IntMap;

// ----------------------------------------------------------------------------

/// Can be shared between different timelines.
pub type Batch<T> = Arc<IntMap<IndexKey, T>>;

#[derive(Default)]
pub struct DataStore {
    data: AHashMap<TypePath, DataPerTypePathTypeErased>,
}

impl DataStore {
    pub fn insert_individual<T: 'static>(
        &mut self,
        type_path: TypePath,
        index_path: IndexPathKey,
        time: TimeValue,
        value: T,
    ) {
        if let Some(store) = self
            .data
            .entry(type_path)
            .or_insert_with(|| DataPerTypePathTypeErased::new_individual::<T>())
            .write::<T>()
        {
            store.insert_individual(index_path, time, value);
        } else {
            panic!("Wrong type!"); // TODO: log warning
        }
    }

    pub fn insert_batch<T: 'static>(
        &mut self,
        type_path: TypePath,
        index_path_prefix: IndexPathKey,
        time: TimeValue,
        values: Batch<T>,
    ) {
        if let Some(store) = self
            .data
            .entry(type_path)
            .or_insert_with(|| DataPerTypePathTypeErased::new_batched::<T>())
            .write::<T>()
        {
            store.insert_batch(index_path_prefix, time, values);
        } else {
            panic!("Wrong type!"); // TODO: log warning
        }
    }

    pub fn get<T: 'static>(&self, type_path: &TypePath) -> Option<&DataPerTypePath<T>> {
        self.data.get(type_path).and_then(|x| x.read::<T>())
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = (&TypePath, &DataPerTypePathTypeErased)> {
        self.data.iter()
    }
}

// ----------------------------------------------------------------------------

/// For a specific [`TypePath`].
///
/// type-erased version of [`DataHistoryT`].
pub struct DataPerTypePathTypeErased(Box<dyn std::any::Any>);

impl DataPerTypePathTypeErased {
    fn new_individual<T: 'static>() -> Self {
        Self(Box::new(DataPerTypePath::<T>::new_individual()))
    }

    fn new_batched<T: 'static>() -> Self {
        Self(Box::new(DataPerTypePath::<T>::new_batched()))
    }

    pub fn read_no_warn<T: 'static>(&self) -> Option<&DataPerTypePath<T>> {
        self.0.downcast_ref::<DataPerTypePath<T>>()
    }

    pub fn read<T: 'static>(&self) -> Option<&DataPerTypePath<T>> {
        if let Some(read) = self.read_no_warn() {
            Some(read)
        } else {
            panic!("Expected {}", std::any::type_name::<T>()); // TODO: just warn
        }
    }

    pub fn write<T: 'static>(&mut self) -> Option<&mut DataPerTypePath<T>> {
        self.0.downcast_mut::<DataPerTypePath<T>>()
    }
}

// ----------------------------------------------------------------------------

pub enum DataPerTypePath<T> {
    /// Individual data at this path.
    Individual(IndividualDataHistory<T>),

    Batched(BatchedDataHistory<T>),
}

impl<T: 'static> DataPerTypePath<T> {
    fn new_individual() -> Self {
        Self::Individual(Default::default())
    }

    fn new_batched() -> Self {
        Self::Batched(Default::default())
    }

    fn as_individual(&mut self) -> &mut IndividualDataHistory<T> {
        match self {
            Self::Individual(individual) => individual,
            Self::Batched(_) => todo!("convert"),
        }
    }

    pub fn insert_individual(&mut self, index_path: IndexPathKey, time: TimeValue, value: T) {
        self.as_individual().insert(index_path, time, value);
    }

    pub fn insert_batch(
        &mut self,
        index_path_prefix: IndexPathKey,
        time: TimeValue,
        values: Batch<T>,
    ) {
        match self {
            Self::Individual(_individual) => {
                todo!("implement slow path");
            }
            Self::Batched(batched) => {
                batched.insert(index_path_prefix, time, values);
            }
        }
    }
}

// ----------------------------------------------------------------------------

/// For a specific [`TypePath`].
pub struct IndividualDataHistory<T> {
    /// fast to find latest value at a certain time.
    values: IntMap<IndexPathKey, BTreeMap<TimeValue, T>>,
}

impl<T> Default for IndividualDataHistory<T> {
    fn default() -> Self {
        Self {
            values: Default::default(),
        }
    }
}

impl<T> IndividualDataHistory<T> {
    pub fn insert(&mut self, index_path: IndexPathKey, time: TimeValue, value: T) {
        self.values
            .entry(index_path)
            .or_default()
            .insert(time, value);
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = (&IndexPathKey, &BTreeMap<TimeValue, T>)> {
        self.values.iter()
    }
}

// ----------------------------------------------------------------------------

/// For a specific [`TypePath`].
pub struct BatchedDataHistory<T> {
    /// The index is the path prefix (everything but the last value).
    batches_over_time: IntMap<IndexPathKey, BTreeMap<TimeValue, Batch<T>>>,
}

impl<T> Default for BatchedDataHistory<T> {
    fn default() -> Self {
        Self {
            batches_over_time: Default::default(),
        }
    }
}

impl<T> BatchedDataHistory<T> {
    pub fn insert(&mut self, index_path_prefix: IndexPathKey, time: TimeValue, values: Batch<T>) {
        self.batches_over_time
            .entry(index_path_prefix)
            .or_default()
            .insert(time, values);
    }

    pub fn iter(
        &self,
    ) -> impl ExactSizeIterator<Item = (&IndexPathKey, &BTreeMap<TimeValue, Batch<T>>)> {
        self.batches_over_time.iter()
    }
}

// ----------------------------------------------------------------------------

pub enum IndividualDataReader<'store, T> {
    None,
    Individual(&'store IndividualDataHistory<T>),
    Batched(&'store BatchedDataHistory<T>),
}

impl<'store, T: 'static> IndividualDataReader<'store, T> {
    pub fn new(store: &'store DataStore, type_path: &TypePath) -> Self {
        if let Some(data) = store.get::<T>(type_path) {
            match data {
                DataPerTypePath::Individual(individual) => Self::Individual(individual),
                DataPerTypePath::Batched(batched) => Self::Batched(batched),
            }
        } else {
            Self::None
        }
    }

    pub fn latest_at(
        &self,
        index_path: &IndexPathKey,
        query_time: &TimeValue,
    ) -> Option<&'store T> {
        match self {
            Self::None => None,
            Self::Individual(history) => {
                latest_at(history.values.get(index_path)?, query_time).map(|(_time, value)| value)
            }
            Self::Batched(data) => {
                let (prefix, suffix) = index_path.split_last();
                latest_at(data.batches_over_time.get(&prefix)?, query_time)?
                    .1
                    .get(&IndexKey::new(suffix))
            }
        }
    }
}

// ----------------------------------------------------------------------------

pub enum BatchedDataReader<'store, T> {
    None,
    Individual(IndexPathKey, TimeValue, &'store IndividualDataHistory<T>),
    Batched(&'store IntMap<IndexKey, T>),
}

impl<'store, T: 'static> BatchedDataReader<'store, T> {
    pub fn new(
        data: Option<&'store DataPerTypePath<T>>,
        index_path_prefix: &IndexPathKey,
        query_time: &TimeValue,
    ) -> Self {
        data.and_then(|data| Self::new_opt(data, index_path_prefix, query_time))
            .unwrap_or(Self::None)
    }

    fn new_opt(
        data: &'store DataPerTypePath<T>,
        index_path_prefix: &IndexPathKey,
        query_time: &TimeValue,
    ) -> Option<Self> {
        match data {
            DataPerTypePath::Individual(individual) => Some(Self::Individual(
                index_path_prefix.clone(),
                *query_time,
                individual,
            )),
            DataPerTypePath::Batched(batched) => {
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
                index_path.push_back(index_path_suffix.index().clone());
                latest_at(history.values.get(&index_path)?, query_time).map(|(_time, value)| value)
            }
            Self::Batched(data) => data.get(index_path_suffix),
        }
    }
}

// ----------------------------------------------------------------------------

fn latest_at<'data, T>(
    data_over_time: &'data BTreeMap<TimeValue, T>,
    query_time: &'_ TimeValue,
) -> Option<(&'data TimeValue, &'data T)> {
    data_over_time.range(..=query_time).rev().next()
}

fn values_in_range<'data, T>(
    data_over_time: &'data BTreeMap<TimeValue, T>,
    time_range: &'_ TimeRange,
) -> impl Iterator<Item = (&'data TimeValue, &'data T)> {
    data_over_time.range(time_range.min..=time_range.max)
}

pub fn query<'data, T>(
    data_over_time: &'data BTreeMap<TimeValue, T>,
    time_query: &TimeQuery,
    mut visit: impl FnMut(&TimeValue, &'data T),
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

pub fn visit_data<'s, T: 'static>(
    store: &'s DataStore,
    time_query: &TimeQuery,
    primary_type_path: &TypePath,
    mut visit: impl FnMut(&'s T),
) -> Option<()> {
    let primary_data = store.get::<T>(primary_type_path)?;

    match primary_data {
        DataPerTypePath::Individual(primary) => {
            for (_index_path, values_over_time) in primary.iter() {
                query(values_over_time, time_query, |_time, primary| {
                    visit(primary);
                });
            }
        }
        DataPerTypePath::Batched(primary) => {
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

pub fn visit_data_and_siblings<'s, T: 'static, S1: 'static>(
    store: &'s DataStore,
    time_query: &TimeQuery,
    primary_type_path: &TypePath,
    (sibling1,): (&str,),
    mut visit: impl FnMut(&'s T, Option<&'s S1>),
) -> Option<()> {
    let primary_data = store.get::<T>(primary_type_path)?;
    let sibling1_path = sibling(primary_type_path, sibling1);

    match primary_data {
        DataPerTypePath::Individual(primary) => {
            let sibling1_reader = IndividualDataReader::<S1>::new(store, &sibling1_path);
            for (index_path, values_over_time) in primary.iter() {
                query(values_over_time, time_query, |time, primary| {
                    let sibling1 = sibling1_reader.latest_at(index_path, time);
                    visit(primary, sibling1);
                });
            }
        }
        DataPerTypePath::Batched(primary) => {
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
