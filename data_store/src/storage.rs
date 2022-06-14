use std::collections::BTreeMap;
use std::sync::Arc;

use ahash::AHashMap;
use nohash_hasher::IntMap;

use log_types::{
    data_types, DataTrait, DataType, DataVec, FieldName, IndexKey, IndexPath, LogId, ObjPath,
};

use crate::{ObjTypePath, TimeQuery};

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
///
/// The [`IndexKey`] is the last path of the [`IndexPath`].
pub type Batch<T> = Arc<IntMap<IndexKey, T>>;

// ----------------------------------------------------------------------------

/// We have one of these per each time source.
pub struct TypePathDataStore<Time> {
    objects: AHashMap<ObjTypePath, ObjStore<Time>>,
}

impl<Time> Default for TypePathDataStore<Time> {
    fn default() -> Self {
        Self {
            objects: Default::default(),
        }
    }
}

impl<Time: 'static + Copy + Ord> TypePathDataStore<Time> {
    pub fn insert_individual<T: DataTrait>(
        &mut self,
        obj_path: ObjPath,
        field_name: FieldName,
        time: Time,
        log_id: LogId,
        value: T,
    ) -> Result<()> {
        let (obj_type_path, index_path) = obj_path.clone().into_type_path_and_index_path();

        self.objects
            .entry(obj_type_path)
            .or_default()
            .insert_individual(index_path, field_name, time, obj_path, log_id, value)
    }

    /// `index_path_prefix` should have `Index::Placeholder` in the last position.
    pub fn insert_batch<T: DataTrait>(
        &mut self,
        parent_obj_path: &ObjPath,
        field_name: FieldName,
        time: Time,
        log_id: LogId,
        batch: Batch<T>,
    ) -> Result<()> {
        crate::profile_function!();

        self.objects
            .entry(parent_obj_path.obj_type_path().clone())
            .or_default()
            .insert_batch(
                parent_obj_path.index_path().clone(),
                field_name,
                time,
                parent_obj_path,
                log_id,
                batch,
            )
    }

    #[inline]
    pub fn get(&self, obj_type_path: &ObjTypePath) -> Option<&ObjStore<Time>> {
        self.objects.get(obj_type_path)
    }

    #[inline]
    pub fn get_field<T: DataTrait>(
        &self,
        obj_type_path: &ObjTypePath,
        field_name: &FieldName,
    ) -> Option<&DataStore<Time, T>> {
        self.get(obj_type_path)
            .and_then(|obj_store| obj_store.get(field_name))
    }

    #[inline]
    pub fn iter(&self) -> impl ExactSizeIterator<Item = (&ObjTypePath, &ObjStore<Time>)> {
        self.objects.iter()
    }
}

// ----------------------------------------------------------------------------

/// One for each time source + [`ObjTypePath`].
///
/// That is, all objects with the same type path are stored here.
pub struct ObjStore<Time> {
    fields: AHashMap<FieldName, DataStoreTypeErased<Time>>,

    /// For each index suffix we know, what is the full object path?
    obj_paths_from_batch_suffix: nohash_hasher::IntMap<IndexKey, ObjPath>,
}

impl<Time> Default for ObjStore<Time> {
    fn default() -> Self {
        Self {
            fields: Default::default(),
            obj_paths_from_batch_suffix: Default::default(),
        }
    }
}

impl<Time: 'static + Copy + Ord> ObjStore<Time> {
    fn insert_individual<T: DataTrait>(
        &mut self,
        index_path: IndexPath,
        field_name: FieldName,
        time: Time,
        obj_path: ObjPath,
        log_id: LogId,
        value: T,
    ) -> Result<()> {
        if let Some(store) = self
            .fields
            .entry(field_name)
            .or_insert_with(|| DataStoreTypeErased::new_individual::<T>())
            .write::<T>()
        {
            store.insert_individual(index_path, time, obj_path, log_id, value)
        } else {
            Err(Error::WrongType)
        }
    }

    fn insert_batch<T: DataTrait>(
        &mut self,
        index_path_prefix: IndexPath,
        field_name: FieldName,
        time: Time,
        parent_obj_path: &ObjPath,
        log_id: LogId,
        batch: Batch<T>,
    ) -> Result<()> {
        for index_path_suffix in batch.keys() {
            if !self
                .obj_paths_from_batch_suffix
                .contains_key(index_path_suffix)
            {
                let obj_path = parent_obj_path
                    .clone()
                    .replace_last_placeholder_with(index_path_suffix.clone());
                self.obj_paths_from_batch_suffix
                    .insert(index_path_suffix.clone(), obj_path);
            }
        }

        if let Some(store) = self
            .fields
            .entry(field_name)
            .or_insert_with(|| DataStoreTypeErased::new_batched::<T>())
            .write::<T>()
        {
            store.insert_batch(index_path_prefix, time, log_id, batch)
        } else {
            Err(Error::WrongType)
        }
    }

    pub fn get_field(&self, field_name: &FieldName) -> Option<&DataStoreTypeErased<Time>> {
        self.fields.get(field_name)
    }

    pub fn get<T: DataTrait>(&self, field_name: &FieldName) -> Option<&DataStore<Time, T>> {
        self.fields
            .get(field_name)
            .and_then(|x| x.read_or_warn::<T>())
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = (&FieldName, &DataStoreTypeErased<Time>)> {
        self.fields.iter()
    }

    pub(crate) fn obj_path_or_die(&self, index_path_suffix: &IndexKey) -> &ObjPath {
        self.obj_paths_from_batch_suffix
            .get(index_path_suffix)
            .unwrap()
    }
}

// ----------------------------------------------------------------------------

/// Type-erased version of [`DataStore`].
pub struct DataStoreTypeErased<Time> {
    data_store: Box<dyn std::any::Any>,
    data_type: DataType,
    _phantom: std::marker::PhantomData<Time>,
}

impl<Time: 'static + Copy + Ord> DataStoreTypeErased<Time> {
    fn new_individual<T: DataTrait>() -> Self {
        Self {
            data_store: Box::new(DataStore::<Time, T>::new_individual()),
            data_type: T::data_typ(),
            _phantom: Default::default(),
        }
    }

    fn new_batched<T: DataTrait>() -> Self {
        Self {
            data_store: Box::new(DataStore::<Time, T>::new_batched()),
            data_type: T::data_typ(),
            _phantom: Default::default(),
        }
    }

    pub fn is<T: DataTrait>(&self) -> bool {
        self.data_store.is::<DataStore<Time, T>>()
    }

    pub fn read_no_warn<T: DataTrait>(&self) -> Option<&DataStore<Time, T>> {
        self.data_store.downcast_ref::<DataStore<Time, T>>()
    }

    pub fn read_or_warn<T: DataTrait>(&self) -> Option<&DataStore<Time, T>> {
        if let Some(read) = self.read_no_warn() {
            Some(read)
        } else {
            tracing::warn!(
                "Expected {} ({:?}), found {:?}",
                std::any::type_name::<T>(),
                T::data_typ(),
                self.data_type
            );
            None
        }
    }

    pub fn write<T: DataTrait>(&mut self) -> Option<&mut DataStore<Time, T>> {
        self.data_store.downcast_mut::<DataStore<Time, T>>()
    }

    /// Typed-erased query of the contents of an object.
    ///
    /// Returns vectors of equal length.
    pub fn query_object(
        &self,
        index_path: IndexPath,
        time_query: &TimeQuery<Time>,
    ) -> (Vec<Time>, Vec<LogId>, DataVec) {
        macro_rules! handle_type(
            ($enum_variant: ident, $typ: ty) => {{
                if let Some(data_store) = self.read_or_warn::<$typ>() {
                    let (times, ids, data) = data_store.query_object(index_path, time_query);
                    (times, ids, DataVec::$enum_variant(data))
                } else {
                    (vec![], vec![], DataVec::$enum_variant(vec![])) // this shouldn't happen
                }
            }}
        );

        match self.data_type {
            DataType::I32 => handle_type!(I32, i32),
            DataType::F32 => handle_type!(F32, f32),
            DataType::String => handle_type!(String, String),
            DataType::Color => handle_type!(Color, data_types::Color),
            DataType::Vec2 => handle_type!(Vec2, data_types::Vec2),
            DataType::BBox2D => handle_type!(BBox2D, log_types::BBox2D),
            DataType::LineSegments2D => handle_type!(LineSegments2D, data_types::LineSegments2D),
            DataType::Image => handle_type!(Image, log_types::Image),
            DataType::Vec3 => handle_type!(Vec3, data_types::Vec3),
            DataType::Box3 => handle_type!(Box3, log_types::Box3),
            DataType::Path3D => handle_type!(Path3D, data_types::Path3D),
            DataType::LineSegments3D => handle_type!(LineSegments3D, data_types::LineSegments3D),
            DataType::Mesh3D => handle_type!(Mesh3D, log_types::Mesh3D),
            DataType::Camera => handle_type!(Camera, log_types::Camera),
            DataType::Vecf32 => handle_type!(Vecf32, Vec<f32>),
            DataType::Space => handle_type!(Space, ObjPath),
        }
    }
}

// ----------------------------------------------------------------------------

pub enum DataStore<Time, T> {
    /// Individual data at this path.
    Individual(IndividualDataHistory<Time, T>),

    Batched(BatchedDataHistory<Time, T>),
}

impl<Time: Copy + Ord, T: DataTrait> DataStore<Time, T> {
    fn new_individual() -> Self {
        Self::Individual(Default::default())
    }

    fn new_batched() -> Self {
        Self::Batched(Default::default())
    }

    fn insert_individual(
        &mut self,
        index_path: IndexPath,
        time: Time,
        obj_path: ObjPath,
        log_id: LogId,
        value: T,
    ) -> Result<()> {
        match self {
            Self::Individual(individual) => {
                individual.insert(index_path, time, obj_path, log_id, value);
                Ok(())
            }
            Self::Batched(_) => Err(Error::BatchFollowedByIndividual),
        }
    }

    fn insert_batch(
        &mut self,
        index_path_prefix: IndexPath,
        time: Time,
        log_id: LogId,
        batch: Batch<T>,
    ) -> Result<()> {
        match self {
            Self::Individual(_) => Err(Error::IndividualFollowedByBatch),
            Self::Batched(batched) => {
                batched.insert(index_path_prefix, time, log_id, batch);
                Ok(())
            }
        }
    }

    /// Returns vectors of equal lengths.
    pub fn query_object(
        &self,
        index_path: IndexPath,
        time_query: &TimeQuery<Time>,
    ) -> (Vec<Time>, Vec<LogId>, Vec<T>) {
        match self {
            Self::Individual(data_history) => data_history.query_object(&index_path, time_query),
            Self::Batched(data_history) => data_history.query_object(index_path, time_query),
        }
    }
}

// ----------------------------------------------------------------------------

/// For a specific [`ObjTypePath`].
pub struct IndividualDataHistory<Time, T> {
    /// fast to find latest value at a certain time.
    pub(crate) values: IntMap<IndexPath, IndividualHistory<Time, T>>,
}

pub struct IndividualHistory<Time, T> {
    /// Path to the parent object.
    ///
    /// This is so that we can quickly check for object visibility when rendering.
    pub(crate) obj_path: ObjPath,
    pub(crate) history: BTreeMap<Time, (LogId, T)>,
}

impl<Time: Copy + Ord, T> Default for IndividualDataHistory<Time, T> {
    fn default() -> Self {
        Self {
            values: Default::default(),
        }
    }
}

impl<Time: Copy + Ord, T> IndividualDataHistory<Time, T> {
    fn insert(
        &mut self,
        index_path: IndexPath,
        time: Time,
        obj_path: ObjPath,
        log_id: LogId,
        value: T,
    ) {
        self.values
            .entry(index_path)
            .or_insert_with(|| IndividualHistory {
                obj_path,
                history: Default::default(),
            })
            .history
            .insert(time, (log_id, value));
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = (&IndexPath, &IndividualHistory<Time, T>)> {
        self.values.iter()
    }
}

impl<Time: Copy + Ord, T: Clone> IndividualDataHistory<Time, T> {
    /// Returns vectors of equal lengths.
    pub fn query_object(
        &self,
        index_path: &IndexPath,
        time_query: &TimeQuery<Time>,
    ) -> (Vec<Time>, Vec<LogId>, Vec<T>) {
        crate::profile_function!();

        let mut times = vec![];
        let mut ids = vec![];
        let mut values = vec![];
        if let Some(history) = self.values.get(index_path) {
            query(&history.history, time_query, |time, (log_id, value)| {
                times.push(*time);
                ids.push(*log_id);
                values.push(value.clone()); // TODO: return references instead
            });
        }
        (times, ids, values)
    }
}

// ----------------------------------------------------------------------------

/// For a specific [`ObjTypePath`].
pub struct BatchedDataHistory<Time, T> {
    /// The index is the path prefix (everything but the last value).
    pub(crate) batches_over_time: IntMap<IndexPath, BatchHistory<Time, T>>,
}

pub struct BatchHistory<Time, T> {
    pub(crate) history: BTreeMap<Time, (LogId, Batch<T>)>,
}

impl<Time: Copy + Ord, T> Default for BatchedDataHistory<Time, T> {
    fn default() -> Self {
        Self {
            batches_over_time: Default::default(),
        }
    }
}

impl<Time: Copy + Ord, T> Default for BatchHistory<Time, T> {
    fn default() -> Self {
        Self {
            history: Default::default(),
        }
    }
}

impl<Time: Copy + Ord, T> BatchedDataHistory<Time, T> {
    fn insert(&mut self, index_path_prefix: IndexPath, time: Time, log_id: LogId, batch: Batch<T>) {
        let batch_history = self.batches_over_time.entry(index_path_prefix).or_default();
        batch_history.history.insert(time, (log_id, batch));
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = (&IndexPath, &BatchHistory<Time, T>)> {
        self.batches_over_time.iter()
    }
}

impl<Time: Copy + Ord, T: Clone> BatchedDataHistory<Time, T> {
    /// Returns vectors of equal lengths.
    pub fn query_object(
        &self,
        index_path: IndexPath,
        time_query: &TimeQuery<Time>,
    ) -> (Vec<Time>, Vec<LogId>, Vec<T>) {
        crate::profile_function!();

        let mut times = vec![];
        let mut ids = vec![];
        let mut values = vec![];

        if index_path.has_placeholder_last() {
            // get all matches
            if let Some(batch_history) = self.batches_over_time.get(&index_path) {
                query(
                    &batch_history.history,
                    time_query,
                    |time, (log_id, batch)| {
                        for value in batch.values() {
                            times.push(*time);
                            ids.push(*log_id);
                            values.push(value.clone()); // TODO: return references instead
                        }
                    },
                );
            }
        } else {
            let (index_path_prefix, index_path_suffix) = index_path.replace_last_with_placeholder();
            if let Some(batch_history) = self.batches_over_time.get(&index_path_prefix) {
                query(
                    &batch_history.history,
                    time_query,
                    |time, (log_id, batch)| {
                        if let Some(value) = batch.get(&index_path_suffix) {
                            times.push(*time);
                            ids.push(*log_id);
                            values.push(value.clone()); // TODO: return references instead
                        }
                    },
                );
            }
        }

        (times, ids, values)
    }
}

// ----------------------------------------------------------------------------

pub(crate) fn latest_at<'data, Time: Copy + Ord, T>(
    data_over_time: &'data BTreeMap<Time, T>,
    query_time: &'_ Time,
) -> Option<(&'data Time, &'data T)> {
    data_over_time.range(..=query_time).rev().next()
}

fn values_in_range<'data, Time: Copy + Ord, T>(
    data_over_time: &'data BTreeMap<Time, T>,
    time_range: &'_ std::ops::RangeInclusive<Time>,
) -> impl Iterator<Item = (&'data Time, &'data T)> {
    data_over_time.range(time_range.start()..=time_range.end())
}

pub(crate) fn query<'data, Time: Copy + Ord, T>(
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

pub(crate) enum IndividualDataReader<'store, Time, T> {
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

pub(crate) enum BatchedDataReader<'store, Time, T> {
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
