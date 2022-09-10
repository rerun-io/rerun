use std::collections::BTreeMap;

use nohash_hasher::IntMap;
use re_log_types::{DataTrait, DataType, DataVec, FieldName, Index, MsgId, ObjPath};

use crate::{BatchOrSplat, Error, Result, TimeQuery};

// ----------------------------------------------------------------------------

/// Stored the data for a specific [`ObjPath`] + [`TimeSource`].
pub struct ObjStore<Time> {
    pub(crate) mono: bool,
    pub(crate) fields: IntMap<FieldName, DataStoreTypeErased<Time>>,
}

impl<Time> Default for ObjStore<Time> {
    fn default() -> Self {
        Self {
            mono: Default::default(),
            fields: Default::default(),
        }
    }
}

impl<Time: 'static + Copy + Ord> ObjStore<Time> {
    pub fn mono(&self) -> bool {
        self.mono
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = (&FieldName, &DataStoreTypeErased<Time>)> {
        self.fields.iter()
    }

    pub fn get(&self, field_name: &FieldName) -> Option<&DataStoreTypeErased<Time>> {
        self.fields.get(field_name)
    }

    pub(crate) fn get_mono<T: DataTrait>(
        &self,
        field_name: &FieldName,
    ) -> Option<&MonoHistory<Time, T>> {
        assert!(self.mono);
        self.fields.get(field_name)?.get_mono::<T>().ok()
    }

    pub(crate) fn get_multi<T: DataTrait>(
        &self,
        field_name: &FieldName,
    ) -> Option<&MultiHistory<Time, T>> {
        assert!(!self.mono);
        self.fields.get(field_name)?.get_multi::<T>().ok()
    }

    pub fn insert_batch<T: DataTrait>(
        &mut self,
        field_name: FieldName,
        time: Time,
        msg_id: MsgId,
        batch: BatchOrSplat<T>,
    ) -> Result<()> {
        if self.fields.is_empty() {
            self.mono = false;
        } else if self.mono {
            return Err(Error::MixingMonoAndMulti);
        }

        let multi = self
            .fields
            .entry(field_name)
            .or_insert_with(|| DataStoreTypeErased::new_multi::<T>())
            .get_multi_mut::<T>()?;

        multi.history.insert(time, (msg_id, batch));

        Ok(())
    }

    pub fn insert_individual<T: DataTrait>(
        &mut self,
        field_name: FieldName,
        time: Time,
        msg_id: MsgId,
        value: T,
    ) -> Result<()> {
        if self.fields.is_empty() {
            self.mono = true;
        } else if !self.mono {
            return Err(Error::MixingMonoAndMulti);
        }

        let mono = self
            .fields
            .entry(field_name)
            .or_insert_with(|| DataStoreTypeErased::new_mono::<T>())
            .get_mono_mut::<T>()?;

        mono.history.insert(time, (msg_id, value));
        Ok(())
    }

    /// Typed-erased query of the contents of one field of this object.
    ///
    /// Returns vectors of equal length.
    pub fn query_field_to_datavec(
        &self,
        field_name: &FieldName,
        time_query: &TimeQuery<Time>,
    ) -> (Vec<(Time, MsgId, Option<Index>)>, DataVec) {
        let store = self.fields.get(field_name).unwrap();
        store.query_field_to_datavec(time_query)
    }
}

// ----------------------------------------------------------------------------

/// Type-erased version of [`DataStore`].
pub struct DataStoreTypeErased<Time> {
    data_store: Box<dyn std::any::Any>,
    mono: bool,
    data_type: DataType,
    _phantom: std::marker::PhantomData<Time>,
}

impl<Time: 'static + Copy + Ord> DataStoreTypeErased<Time> {
    fn new_mono<T: DataTrait>() -> Self {
        Self {
            data_store: Box::new(MonoHistory::<Time, T>::default()),
            mono: true,
            data_type: T::data_typ(),
            _phantom: Default::default(),
        }
    }

    fn new_multi<T: DataTrait>() -> Self {
        Self {
            data_store: Box::new(MultiHistory::<Time, T>::default()),
            mono: false,
            data_type: T::data_typ(),
            _phantom: Default::default(),
        }
    }

    pub(crate) fn get_mono<T: DataTrait>(&self) -> Result<&MonoHistory<Time, T>> {
        if let Some(history) = self.data_store.downcast_ref::<MonoHistory<Time, T>>() {
            Ok(history)
        } else if !self.mono {
            Err(Error::MixingMonoAndMulti)
        } else if self.data_type != T::data_typ() {
            Err(Error::MixingTypes {
                existing: self.data_type,
                expected: T::data_typ(),
            })
        } else {
            unreachable!("Correct mono/multi and data-type, buy Any still fails to cast");
        }
    }

    pub(crate) fn get_multi<T: DataTrait>(&self) -> Result<&MultiHistory<Time, T>> {
        if let Some(history) = self.data_store.downcast_ref::<MultiHistory<Time, T>>() {
            Ok(history)
        } else if self.mono {
            Err(Error::MixingMonoAndMulti)
        } else if self.data_type != T::data_typ() {
            Err(Error::MixingTypes {
                existing: self.data_type,
                expected: T::data_typ(),
            })
        } else {
            unreachable!("Correct mono/multi and data-type, buy Any still fails to cast");
        }
    }

    pub(crate) fn get_mono_mut<T: DataTrait>(&mut self) -> Result<&mut MonoHistory<Time, T>> {
        if let Some(history) = self.data_store.downcast_mut::<MonoHistory<Time, T>>() {
            Ok(history)
        } else if !self.mono {
            Err(Error::MixingMonoAndMulti)
        } else if self.data_type != T::data_typ() {
            Err(Error::MixingTypes {
                existing: self.data_type,
                expected: T::data_typ(),
            })
        } else {
            unreachable!("Correct mono/multi and data-type, buy Any still fails to cast");
        }
    }

    pub(crate) fn get_multi_mut<T: DataTrait>(&mut self) -> Result<&mut MultiHistory<Time, T>> {
        if let Some(history) = self.data_store.downcast_mut::<MultiHistory<Time, T>>() {
            Ok(history)
        } else if self.mono {
            Err(Error::MixingMonoAndMulti)
        } else if self.data_type != T::data_typ() {
            Err(Error::MixingTypes {
                existing: self.data_type,
                expected: T::data_typ(),
            })
        } else {
            unreachable!("Correct mono/multi and data-type, buy Any still fails to cast");
        }
    }

    /// Typed-erased query of the contents of one field of this object.
    ///
    /// Returns vectors of equal length.
    pub fn query_field_to_datavec(
        &self,
        time_query: &TimeQuery<Time>,
    ) -> (Vec<(Time, MsgId, Option<Index>)>, DataVec) {
        macro_rules! handle_type(
            ($enum_variant: ident, $typ: ty) => {{
                let mut time_msgid_index = vec![];
                let mut values = vec![];
                if self.mono {
                    let mono = self.get_mono::<$typ>().unwrap();
                    mono.query(time_query, |time, msg_id, value| {
                        time_msgid_index.push((*time, *msg_id, None));
                        values.push(value.clone());
                    });
                } else {
                    let multi = self.get_multi::<$typ>().unwrap();
                    multi.query(time_query, |time, msg_id, batch| {
                        match batch {
                            BatchOrSplat::Splat(value) => {
                                time_msgid_index.push((*time, *msg_id, None));
                                values.push(value.clone());
                            }
                            BatchOrSplat::Batch(batch) => {
                                for (index_hash, index) in batch.indices() {
                                    let value = batch.get(index_hash).unwrap();
                                    time_msgid_index.push((*time, *msg_id, Some(index.clone())));
                                    values.push(value.clone());
                                }
                            }
                        }
                    });
                }
                (time_msgid_index, DataVec::$enum_variant(values))
            }}
        );

        use re_log_types::data_types;

        match self.data_type {
            DataType::I32 => handle_type!(I32, i32),
            DataType::F32 => handle_type!(F32, f32),
            DataType::String => handle_type!(String, String),
            DataType::Color => handle_type!(Color, data_types::Color),
            DataType::Vec2 => handle_type!(Vec2, data_types::Vec2),
            DataType::BBox2D => handle_type!(BBox2D, re_log_types::BBox2D),
            DataType::Vec3 => handle_type!(Vec3, data_types::Vec3),
            DataType::Box3 => handle_type!(Box3, re_log_types::Box3),
            DataType::Mesh3D => handle_type!(Mesh3D, re_log_types::Mesh3D),
            DataType::Camera => handle_type!(Camera, re_log_types::Camera),
            DataType::Tensor => handle_type!(Tensor, re_log_types::Tensor),
            DataType::Space => handle_type!(Space, ObjPath),
            DataType::DataVec => handle_type!(DataVec, DataVec),
        }
    }
}

// ----------------------------------------------------------------------------

pub(crate) struct MonoHistory<Time, T> {
    pub(crate) history: BTreeMap<Time, (MsgId, T)>,
}

impl<Time, T> Default for MonoHistory<Time, T> {
    fn default() -> Self {
        Self {
            history: Default::default(),
        }
    }
}

impl<Time: 'static + Copy + Ord, T: DataTrait> MonoHistory<Time, T> {
    pub fn query<'slf>(
        &'slf self,
        time_query: &TimeQuery<Time>,
        mut visit: impl FnMut(&Time, &MsgId, &'slf T),
    ) {
        crate::query::query(&self.history, time_query, |time, (msg_id, value)| {
            visit(time, msg_id, value);
        });
    }
}

// ----------------------------------------------------------------------------

pub(crate) struct MultiHistory<Time, T> {
    pub(crate) history: BTreeMap<Time, (MsgId, BatchOrSplat<T>)>,
}

impl<Time, T> Default for MultiHistory<Time, T> {
    fn default() -> Self {
        Self {
            history: Default::default(),
        }
    }
}

impl<Time: 'static + Copy + Ord, T: DataTrait> MultiHistory<Time, T> {
    pub fn query<'slf>(
        &'slf self,
        time_query: &TimeQuery<Time>,
        mut visit: impl FnMut(&Time, &MsgId, &'slf BatchOrSplat<T>),
    ) {
        crate::query::query(&self.history, time_query, |time, (msg_id, batch)| {
            visit(time, msg_id, batch);
        });
    }
}

// ----------------------------------------------------------------------------

#[test]
fn test_obj_store() {
    let mut obj_store = ObjStore::default();

    assert!(obj_store
        .insert_individual("field1".into(), 0, MsgId::random(), 3.15)
        .is_ok());

    assert!(obj_store
        .insert_individual("field1".into(), 0, MsgId::random(), 3.15)
        .is_ok());

    assert!(obj_store
        .insert_individual("field2".into(), 0, MsgId::random(), 3.15)
        .is_ok());

    assert_eq!(
        obj_store.insert_individual("field2".into(), 0, MsgId::random(), 42),
        Err(crate::Error::MixingTypes {
            existing: DataType::F32,
            expected: DataType::I32
        })
    );

    let batch = crate::BatchOrSplat::Splat(42.0);
    assert_eq!(
        obj_store.insert_batch("field3".into(), 0, MsgId::random(), batch),
        Err(crate::Error::MixingMonoAndMulti)
    );
}
