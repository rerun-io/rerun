use crate::*;

use std::collections::BTreeMap;

use ahash::AHashMap;
use nohash_hasher::IntMap;

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
            // TODO: log warning
        }
    }

    pub fn insert_batch<T: 'static>(
        &mut self,
        type_path: TypePath,
        index_path_prefix: IndexPathKey,
        time: TimeValue,
        values: impl Iterator<Item = (Index, T)>,
    ) {
        if let Some(store) = self
            .data
            .entry(type_path)
            .or_insert_with(|| DataPerTypePathTypeErased::new_batched::<T>())
            .write::<T>()
        {
            store.insert_batch(index_path_prefix, time, values);
        } else {
            // TODO: log warning
        }
    }
}

/// For a specific [`TypePath`].
///
/// type-erased version of [`DataHistoryT`].
struct DataPerTypePathTypeErased(Box<dyn std::any::Any>);

impl DataPerTypePathTypeErased {
    pub fn new_individual<T: 'static>() -> Self {
        Self(Box::new(DataPerTypePath::<T>::new_individual()))
    }

    pub fn new_batched<T: 'static>() -> Self {
        Self(Box::new(DataPerTypePath::<T>::new_batched()))
    }

    pub fn read<T: 'static>(&self) -> Option<&DataPerTypePath<T>> {
        self.0.downcast_ref::<DataPerTypePath<T>>()
    }

    pub fn write<T: 'static>(&mut self) -> Option<&mut DataPerTypePath<T>> {
        self.0.downcast_mut::<DataPerTypePath<T>>()
    }
}

enum DataPerTypePath<T> {
    /// Individual data at this path.
    Individual(DataHistory<T>),

    Batched(BatchedDataHistory<T>),
}

impl<T: 'static> DataPerTypePath<T> {
    pub fn new_individual() -> Self {
        Self::Individual(Default::default())
    }

    pub fn new_batched() -> Self {
        Self::Batched(Default::default())
    }

    fn as_individual(&mut self) -> &mut DataHistory<T> {
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
        values: impl Iterator<Item = (Index, T)>,
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

/// For a specific [`TypePath`].
pub struct DataHistory<T> {
    /// fast to find latest value at a certain time.
    values: IntMap<IndexPathKey, BTreeMap<TimeValue, T>>,
}

impl<T> Default for DataHistory<T> {
    fn default() -> Self {
        Self {
            values: Default::default(),
        }
    }
}

impl<T> DataHistory<T> {
    pub fn insert(&mut self, index_path: IndexPathKey, time: TimeValue, value: T) {
        self.values
            .entry(index_path)
            .or_default()
            .insert(time, value);
    }
}

// ----------------------------------------------------------------------------

/// For a specific [`TypePath`].
pub struct BatchedDataHistory<T> {
    /// The index is the path prefix (everything but the last value).
    batches_over_time: IntMap<IndexPathKey, BTreeMap<TimeValue, AHashMap<Index, T>>>,
}

impl<T> Default for BatchedDataHistory<T> {
    fn default() -> Self {
        Self {
            batches_over_time: Default::default(),
        }
    }
}

impl<T> BatchedDataHistory<T> {
    pub fn insert(
        &mut self,
        index_path_prefix: IndexPathKey,
        time: TimeValue,
        values: impl Iterator<Item = (Index, T)>,
    ) {
        let time_slot = self
            .batches_over_time
            .entry(index_path_prefix)
            .or_default()
            .entry(time)
            .or_default();
        for (index_suffix, data) in values {
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
pub struct Point3<'s> {
    pos: &'s [f32; 3],
    radius: Option<f32>,
}

#[derive(Default)]
pub struct Scene3D<'s> {
    pub points: Vec<Point3<'s>>,
}

impl<'s> Scene3D<'s> {
    pub fn from_store(store: &'s DataStore, time_query: &TimeQuery) -> Self {
        let mut slf = Self::default();

        for (type_path, data) in &store.data {
            if type_path.last() == Some(&TypePathComponent::Name("pos".into())) {
                if let Some(pos_data) = data.read::<[f32; 3]>() {
                    Self::collect_points(&mut slf.points, store, time_query, type_path, pos_data);
                }
            }
        }

        slf
    }

    fn collect_points(
        out_points: &mut Vec<Point3<'s>>,
        store: &'s DataStore,
        time_query: &TimeQuery,
        type_path: &TypePath,
        pos_data: &'s DataPerTypePath<[f32; 3]>,
    ) -> Option<()> {
        let radius_path = sibling(type_path, "radius");

        match pos_data {
            DataPerTypePath::Individual(pos) => {
                let radius = IndividualDataReader::<f32>::new(store, &radius_path)
                    .unwrap_or(IndividualDataReader::None);

                for (index_path, values_over_time) in &pos.values {
                    match time_query {
                        TimeQuery::LatestAt(query_time) => {
                            if let Some((_pos_time, pos)) = latest_at(values_over_time, query_time) {
                                out_points.push(Point3 {
                                    pos,
                                    radius: radius.get_latest_at(index_path, query_time).copied(),
                                });
                            }
                        }
                        TimeQuery::Range(query_range) => {
                            for (pos_time, pos) in values_in_range(values_over_time, query_range) {
                                out_points.push(Point3 {
                                    pos,
                                    radius: radius.get_latest_at(index_path, pos_time).copied(),
                                });
                            }
                        }
                    }
                }
            }
            DataPerTypePath::Batched(pos) => {
                for (index_path_prefix, pos) in &pos.batches_over_time {
                    let radius = store.data.get(&radius_path).and_then(|x| x.read::<f32>());

                    match time_query {
                        TimeQuery::LatestAt(query_time) => {
                            if let Some((_, pos)) = latest_at(pos, query_time) {
                                let radius =
                                    batch_data_reader(radius, index_path_prefix, query_time);

                                for (index_path_suffix, pos) in pos {
                                    out_points.push(Point3 {
                                        pos,
                                        radius: radius.get_latest_at(index_path_suffix).copied(),
                                    });
                                }
                            }
                        }
                        TimeQuery::Range(query_range) => {
                            for (pos_time, pos) in values_in_range(pos, query_range) {
                                let radius = batch_data_reader(radius, index_path_prefix, pos_time);

                                for (index_path_suffix, pos) in pos {
                                    out_points.push(Point3 {
                                        pos,
                                        radius: radius.get_latest_at(index_path_suffix).copied(),
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }

        Some(())
    }
}

fn sibling(type_path: &TypePath, name: &str) -> TypePath {
    let mut type_path = type_path.clone();
    type_path.pop_back();
    type_path.push_back(TypePathComponent::Name(name.into()));
    type_path
}

fn batch_data_reader<'store, T: 'static>(
    data: Option<&'store DataPerTypePath<T>>,
    index_path_prefix: &IndexPathKey,
    query_time: &TimeValue,
) -> BatchedDataReader<'store, T> {
    data.and_then(|data| BatchedDataReader::new(data, index_path_prefix, query_time))
        .unwrap_or(BatchedDataReader::None)
}

// ----------------------------------------------------------------------------

enum IndividualDataReader<'store, T> {
    None,
    Individual(&'store DataHistory<T>),
    Batched(&'store BatchedDataHistory<T>),
}

impl<'store, T: 'static> IndividualDataReader<'store, T> {
    pub fn new(store: &'store DataStore, type_path: &TypePath) -> Option<Self> {
        let data = store.data.get(type_path)?.read::<T>()?;
        match data {
            DataPerTypePath::Individual(individual) => Some(Self::Individual(individual)),
            DataPerTypePath::Batched(batched) => Some(Self::Batched(batched)),
        }
    }

    pub fn get_latest_at(&self, index_path: &IndexPathKey, query_time: &TimeValue) -> Option<&T> {
        match self {
            Self::None => None,
            Self::Individual(history) => {
                latest_at(history.values.get(index_path)?, query_time).map(|(_time, value)| value)
            }
            Self::Batched(data) => {
                let (prefix, suffix) = index_path.split_last();
                latest_at(data.batches_over_time.get(&prefix)?, query_time)?
                    .1
                    .get(&suffix)
            }
        }
    }
}

// ----------------------------------------------------------------------------

enum BatchedDataReader<'store, T> {
    None,
    Individual(IndexPathKey, TimeValue, &'store DataHistory<T>),
    Batched(&'store AHashMap<Index, T>),
}

impl<'store, T: 'static> BatchedDataReader<'store, T> {
    pub fn new(
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

    pub fn get_latest_at(&self, index_path_suffix: &Index) -> Option<&T> {
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

#[test]
fn test_data_storage() {
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
        TimeValue::Sequence(1),
        Data::Pos3([1.0, 2.0, 3.0]),
    );

    let (type_path, index_path) = into_type_path(data_path(0, "radius"));
    store.insert_individual(
        type_path,
        index_path,
        TimeValue::Sequence(2),
        Data::F32(1.0),
    );

    assert_eq!(
        Scene3D::from_store(&store, &TimeQuery::LatestAt(TimeValue::Sequence(0))).points,
        vec![]
    );

    assert_eq!(
        Scene3D::from_store(&store, &TimeQuery::LatestAt(TimeValue::Sequence(1))).points,
        vec![Point3 {
            pos: &[1.0, 2.0, 3.0],
            radius: None
        }]
    );

    assert_eq!(
        Scene3D::from_store(&store, &TimeQuery::LatestAt(TimeValue::Sequence(2))).points,
        vec![Point3 {
            pos: &[1.0, 2.0, 3.0],
            radius: Some(1.0)
        }]
    );
}
