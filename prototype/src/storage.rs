use crate::*;

use std::collections::BTreeMap;

use ahash::AHashMap;
use nohash_hasher::IntMap;

#[derive(Default)]
pub struct DataStore {
    data: AHashMap<TypePath, DataPerTypePath>,
}

#[derive(Default)]
struct DataPerTypePath {
    // TODO: make this an enum
    /// Individual data at this path.
    individual: DataHistory,

    /// The index path prefix (everything but the last value).
    batched: IntMap<IndexPathKey, BatchedDataHistory>,
}

impl DataStore {
    pub fn insert_individual(
        &mut self,
        type_path: TypePath,
        index_path: IndexPathKey,
        time_stamp: TimeStamp,
        data: Data,
    ) {
        self.data
            .entry(type_path)
            .or_default()
            .insert_individual(index_path, time_stamp, data)
    }

    pub fn insert_batch<T: 'static + Clone>(
        &mut self,
        type_path: TypePath,
        index_path_prefix: IndexPathKey,
        time_stamp: TimeStamp,
        data: impl Iterator<Item = (Index, T)> + Clone,
    ) {
        self.data
            .entry(type_path)
            .or_default()
            .insert_batch(index_path_prefix, time_stamp, data)
    }
}

impl DataPerTypePath {
    pub fn insert_individual(&mut self, index_path: IndexPathKey, time_stamp: TimeStamp, data: Data) {
        self.individual.insert(index_path, time_stamp, data);
    }

    pub fn insert_batch<T: 'static + Clone>(
        &mut self,
        index_path_prefix: IndexPathKey,
        time_stamp: TimeStamp,
        data: impl Iterator<Item = (Index, T)> + Clone,
    ) {
        self.batched
            .entry(index_path_prefix)
            .or_default()
            .insert(time_stamp, data);
    }
}

/// For a specific [`TypePath`].
///
/// type-erased version of [`DataHistoryT`].
#[derive(Default)]
pub struct DataHistory(Option<Box<dyn std::any::Any>>);

impl DataHistory {
    pub fn read<T: 'static>(&self) -> Option<&DataHistoryT<T>> {
        self.0
            .as_ref()
            .and_then(|any| any.downcast_ref::<DataHistoryT<T>>())
    }

    pub fn write<T: 'static>(&mut self) -> Option<&mut DataHistoryT<T>> {
        self.0
            .get_or_insert_with(|| Box::new(DataHistoryT::<T>::default()))
            .downcast_mut::<DataHistoryT<T>>()
    }

    pub fn insert(&mut self, index_path: IndexPathKey, time_stamp: TimeStamp, data: Data) {
        match data {
            Data::F32(value) => {
                if let Some(data_store) = self.write::<f32>() {
                    data_store.insert(index_path, time_stamp, value);
                } else {
                    // TODO: log warning
                }
            }
            Data::Pos3(value) => {
                if let Some(data_store) = self.write::<[f32; 3]>() {
                    data_store.insert(index_path, time_stamp, value);
                } else {
                    // TODO: log warning
                }
            }
        }
    }
}

/// For a specific [`TypePath`].
pub struct DataHistoryT<T> {
    per_time_source: BTreeMap<TimeSource, DataPerTimeSource<T>>,
}

impl<T> Default for DataHistoryT<T> {
    fn default() -> Self {
        Self {
            per_time_source: Default::default(),
        }
    }
}

impl<T: Clone> DataHistoryT<T> {
    pub fn insert(&mut self, index_path: IndexPathKey, time_stamp: TimeStamp, data: T) {
        // TODO: optimize away clones for the case when time_stamp.0.len() == 1
        for (time_source, time_value) in time_stamp.0 {
            self.per_time_source.entry(time_source).or_default().insert(
                index_path.clone(),
                time_value,
                data.clone(),
            );
        }
    }
}

struct DataPerTimeSource<T> {
    /// fast to find latest value at a certain time.
    values: IntMap<IndexPathKey, BTreeMap<TimeValue, T>>,
}

impl<T> Default for DataPerTimeSource<T> {
    fn default() -> Self {
        Self {
            values: Default::default(),
        }
    }
}

impl<T: Clone> DataPerTimeSource<T> {
    pub fn insert(&mut self, index_path: IndexPathKey, data_time: TimeValue, data: T) {
        self.values
            .entry(index_path)
            .or_default()
            .insert(data_time, data);
    }
}

// ----------------------------------------------------------------------------

/// For a specific [`TypePath`].
///
/// type-erased version of [`BatchedDataHistoryT`].
#[derive(Default)]
pub struct BatchedDataHistory(Option<Box<dyn std::any::Any>>);

impl BatchedDataHistory {
    pub fn read<T: 'static>(&self) -> Option<&BatchedDataHistoryT<T>> {
        self.0
            .as_ref()
            .and_then(|any| any.downcast_ref::<BatchedDataHistoryT<T>>())
    }

    pub fn write<T: 'static>(&mut self) -> Option<&mut BatchedDataHistoryT<T>> {
        self.0
            .get_or_insert_with(|| Box::new(BatchedDataHistoryT::<T>::default()))
            .downcast_mut::<BatchedDataHistoryT<T>>()
    }

    pub fn insert<T: 'static + Clone>(
        &mut self,
        time_stamp: TimeStamp,
        data: impl Iterator<Item = (Index, T)> + Clone,
    ) {
        if let Some(data_store) = self.write::<T>() {
            data_store.insert(time_stamp, data);
        } else {
            // TODO: log warning
        }
    }
}

/// For a specific [`TypePath`].
pub struct BatchedDataHistoryT<T> {
    per_time_source: BTreeMap<TimeSource, BatchedDataPerTimeSource<T>>,
}

impl<T> Default for BatchedDataHistoryT<T> {
    fn default() -> Self {
        Self {
            per_time_source: Default::default(),
        }
    }
}

impl<T: Clone> BatchedDataHistoryT<T> {
    pub fn insert(
        &mut self,
        time_stamp: TimeStamp,
        data: impl Iterator<Item = (Index, T)> + Clone,
    ) {
        // TODO: optimize away clones for the case when time_stamp.0.len() == 1
        for (time_source, time_value) in time_stamp.0 {
            self.per_time_source
                .entry(time_source)
                .or_default()
                .insert(time_value, data.clone());
        }
    }
}

struct BatchedDataPerTimeSource<T> {
    everything_per_time: BTreeMap<TimeValue, AHashMap<Index, T>>,
}

impl<T> Default for BatchedDataPerTimeSource<T> {
    fn default() -> Self {
        Self {
            everything_per_time: Default::default(),
        }
    }
}

impl<T: Clone> BatchedDataPerTimeSource<T> {
    pub fn insert(&mut self, data_time: TimeValue, data: impl Iterator<Item = (Index, T)> + Clone) {
        let time_slot = self.everything_per_time.entry(data_time).or_default();
        for (index_suffix, data) in data {
            time_slot.insert(index_suffix, data);
        }
    }
}

// ----------------------------------------------------------------------------

#[inline(never)] // better profiling
fn latest_at<'data, T>(
    data_over_time: &'data BTreeMap<TimeValue, T>,
    query_time: &'_ TimeValue,
) -> Option<(&'data TimeValue, &'data T)> {
    data_over_time.range(..=query_time).rev().next()
}

#[inline(never)] // better profiling
fn values_in_range<'data, T>(
    data_over_time: &'data BTreeMap<TimeValue, T>,
    time_range: &'_ TimeRange,
) -> impl Iterator<Item = (&'data TimeValue, &'data T)> {
    data_over_time.range(time_range.min..=time_range.max)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point3 {
    pos: [f32; 3],
    radius: Option<f32>,
}

#[derive(Default)]
pub struct Scene3D {
    pub points: Vec<Point3>,
}

impl Scene3D {
    pub fn from_store(store: &DataStore, time_source: &TimeSource, query_time: &TimeValue) -> Self {
        let mut points = vec![];

        for (type_path, data) in &store.data {
            if type_path.last() == Some(&TypePathComponent::Name("pos".into())) {
                let radius_path = sibling(type_path, "radius");

                if let Some(pos) = data.individual.read::<[f32; 3]>() {
                    if let Some(pos) = pos.per_time_source.get(time_source) {
                        let radius = IndividualDataReader::<f32>::new(store, &radius_path, time_source);

                        for (index_path, values_over_time) in &pos.values {
                            if let Some((_, pos)) = latest_at(values_over_time, query_time) {
                                points.push(Point3 {
                                    pos: *pos,
                                    radius: radius
                                        .as_ref()
                                        .and_then(|radius| {
                                            radius.get_latest_at(index_path, query_time)
                                        })
                                        .copied(),
                                });
                            }
                        }
                    }
                }
                for (index_path_prefix, batched) in &data.batched {
                    if let Some(pos) = batched.read::<[f32; 3]>() {
                        if let Some(pos) = pos.per_time_source.get(time_source) {
                            if let Some((_, pos)) = latest_at(&pos.everything_per_time, query_time)
                            {
                                let radius = BatchedDataReader::new(
                                    store,
                                    &radius_path,
                                    index_path_prefix,
                                    time_source,
                                    query_time,
                                );

                                for (index_path_suffix, pos) in pos {
                                    points.push(Point3 {
                                        pos: *pos,
                                        radius: radius
                                            .as_ref()
                                            .and_then(|radius| {
                                                radius.get_latest_at(index_path_suffix)
                                            })
                                            .copied(),
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }

        Self { points }
    }
}

fn sibling(type_path: &TypePath, name: &str) -> TypePath {
    let mut type_path = type_path.clone();
    type_path.pop_back();
    type_path.push_back(TypePathComponent::Name(name.into()));
    type_path
}

// ----------------------------------------------------------------------------

enum IndividualDataReader<'store, T> {
    Individual(&'store DataPerTimeSource<T>),
    Batched(TimeSource, &'store IntMap<IndexPathKey, BatchedDataHistory>),
}

impl<'store, T: 'static> IndividualDataReader<'store, T> {
    pub fn new(
        store: &'store DataStore,
        type_path: &TypePath,
        time_source: &TimeSource,
    ) -> Option<Self> {
        let data = store.data.get(type_path)?;
        if let Some(data) = data.individual.read::<T>() {
            if let Some(data) = data.per_time_source.get(time_source) {
                return Some(Self::Individual(data));
            }
        }
        Some(Self::Batched(time_source.clone(), &data.batched))
    }

    pub fn get_latest_at(&self, index_path: &IndexPathKey, query_time: &TimeValue) -> Option<&T> {
        match self {
            Self::Individual(history) => {
                latest_at(history.values.get(index_path)?, query_time).map(|(_time, value)| value)
            }
            Self::Batched(time_source, data) => {
                let (prefix, suffix) = index_path.split_last();
                latest_at(
                    &data
                        .get(&prefix)?
                        .read::<T>()?
                        .per_time_source
                        .get(time_source)?
                        .everything_per_time,
                    query_time,
                )?
                .1
                .get(&suffix)
            }
        }
    }
}

// ----------------------------------------------------------------------------

enum BatchedDataReader<'store, T> {
    Individual(IndexPathKey, TimeValue, &'store DataPerTimeSource<T>),
    Batched(&'store AHashMap<Index, T>),
}

impl<'store, T: 'static> BatchedDataReader<'store, T> {
    pub fn new(
        store: &'store DataStore,
        type_path: &TypePath,
        index_path_prefix: &IndexPathKey,
        time_source: &TimeSource,
        query_time: &TimeValue,
    ) -> Option<Self> {
        let data = store.data.get(type_path)?;
        if let Some(data) = data.individual.read::<T>() {
            if let Some(data) = data.per_time_source.get(time_source) {
                return Some(Self::Individual(index_path_prefix.clone(), *query_time, data));
            }
        }
        let (_, map) = latest_at(
            &data
                .batched
                .get(index_path_prefix)?
                .read::<T>()?
                .per_time_source
                .get(time_source)?
                .everything_per_time,
            query_time,
        )?;
        Some(Self::Batched(map))
    }

    pub fn get_latest_at(&self, index_path_suffix: &Index) -> Option<&T> {
        match self {
            Self::Individual(index_path_prefix, query_time, history) => {
                let mut index_path = index_path_prefix.clone();
                index_path.push_back(index_path_suffix.clone());
                latest_at(history.values.get(&index_path)?, query_time).map(|(_time, value)| value)
            }
            Self::Batched(data) => data.get(&index_path_suffix),
        }
    }
}

// ----------------------------------------------------------------------------

#[test]
fn test_data_storage() {
    fn time_stamp(seq: i64) -> TimeStamp {
        let mut time_stamp = TimeStamp::default();
        time_stamp
            .0
            .insert("frame".to_string(), TimeValue::Sequence(seq));
        time_stamp
    }
    fn data_path(index: u64, field: &str) -> DataPath {
        im::vector![
            DataPathComponent::Name("camera".into()),
            DataPathComponent::Index(Index::String("left".into())),
            DataPathComponent::Name("point".into()),
            DataPathComponent::Index(Index::Sequence(index)),
            DataPathComponent::Name(field.into()),
        ]
    }

    let mut store = DataStore::default();

    let (type_path, index_path) = into_type_path(data_path(0, "pos"));
    store.insert_individual(
        type_path,
        index_path,
        time_stamp(1),
        Data::Pos3([1.0, 2.0, 3.0]),
    );

    let (type_path, index_path) = into_type_path(data_path(0, "radius"));
    store.insert_individual(type_path, index_path, time_stamp(2), Data::F32(1.0));

    assert_eq!(
        Scene3D::from_store(&store, &"frame".to_string(), &TimeValue::Sequence(0),).points,
        vec![]
    );

    assert_eq!(
        Scene3D::from_store(&store, &"frame".to_string(), &TimeValue::Sequence(1),).points,
        vec![Point3 {
            pos: [1.0, 2.0, 3.0],
            radius: None
        }]
    );

    assert_eq!(
        Scene3D::from_store(&store, &"frame".to_string(), &TimeValue::Sequence(2),).points,
        vec![Point3 {
            pos: [1.0, 2.0, 3.0],
            radius: Some(1.0)
        }]
    );
}
