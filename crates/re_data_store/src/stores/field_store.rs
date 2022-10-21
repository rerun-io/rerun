use std::collections::BTreeMap;

use re_log_types::{DataTrait, DataType, DataVec, Index, MsgId, ObjPath};

use crate::{BatchOrSplat, Error, Result, TimeQuery};

// ----------------------------------------------------------------------------

/// Two equally long vectors.
///
/// First has time, message id, and the multi-index (if any).
/// Second has the matching data.
pub type FieldQueryOutput<Time> = (Vec<(Time, MsgId)>, DataVec);

/// Stores data for a specific [`re_log_types::FieldName`] of a specific [`ObjPath`] on a specific [`re_log_types::Timeline`].
pub struct FieldStore<Time> {
    data_store: Box<dyn std::any::Any>,
    mono: bool,
    data_type: DataType,
    _phantom: std::marker::PhantomData<Time>,
}

impl<Time: 'static + Copy + Ord> FieldStore<Time> {
    pub(crate) fn new_mono<T: DataTrait>() -> Self {
        Self {
            data_store: Box::new(MonoFieldStore::<Time, T>::default()),
            mono: true,
            data_type: T::data_typ(),
            _phantom: Default::default(),
        }
    }

    pub(crate) fn new_multi<T: DataTrait>() -> Self {
        Self {
            data_store: Box::new(MultiFieldStore::<Time, T>::default()),
            mono: false,
            data_type: T::data_typ(),
            _phantom: Default::default(),
        }
    }

    pub fn get_mono<T: DataTrait>(&self) -> Result<&MonoFieldStore<Time, T>> {
        if let Some(history) = self.data_store.downcast_ref::<MonoFieldStore<Time, T>>() {
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

    pub(crate) fn get_multi<T: DataTrait>(&self) -> Result<&MultiFieldStore<Time, T>> {
        if let Some(history) = self.data_store.downcast_ref::<MultiFieldStore<Time, T>>() {
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

    pub(crate) fn get_mono_mut<T: DataTrait>(&mut self) -> Result<&mut MonoFieldStore<Time, T>> {
        if let Some(history) = self.data_store.downcast_mut::<MonoFieldStore<Time, T>>() {
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

    pub(crate) fn get_multi_mut<T: DataTrait>(&mut self) -> Result<&mut MultiFieldStore<Time, T>> {
        if let Some(history) = self.data_store.downcast_mut::<MultiFieldStore<Time, T>>() {
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
    /// If `instance_index` is `None`, all instances are returned.
    /// If `instance_index` is `Some`, only those instances that match will be returned.
    ///
    /// Returns vectors of equal length.
    pub fn query_field_to_datavec(
        &self,
        time_query: &TimeQuery<Time>,
        instance_index: Option<&Index>,
    ) -> Result<FieldQueryOutput<Time>> {
        macro_rules! handle_type(
            ($enum_variant: ident, $typ: ty) => {{
                let mut time_msgid_index = vec![];
                let mut values = vec![];
                if self.mono {
                    if instance_index.is_some() {
                        return Err(Error::MixingMonoAndMulti);
                    }

                    let mono = self.get_mono::<$typ>()?;
                    mono.query(time_query, |time, msg_id, value| {
                        time_msgid_index.push((*time, *msg_id));
                        values.push(value.clone());
                    });
                } else {
                    let multi = self.get_multi::<$typ>()?;
                    multi.query(time_query, |time, msg_id, batch| {
                        match batch {
                            BatchOrSplat::Splat(value) => {
                                time_msgid_index.push((*time, *msg_id));
                                values.push(value.clone());
                            }
                            BatchOrSplat::Batch(batch) => {
                                if let Some(index) = instance_index {
                                    let value = batch.get_index(index).expect("Batches should be self-consistent");
                                    time_msgid_index.push((*time, *msg_id));
                                    values.push(value.clone());
                                } else {
                                    for (_index_hash, value) in batch.iter() {
                                        time_msgid_index.push((*time, *msg_id));
                                        values.push(value.clone());
                                    }
                                }
                            }
                        }
                    });
                }
                Ok((time_msgid_index, DataVec::$enum_variant(values)))
            }}
        );

        use re_log_types::data_types;

        match self.data_type {
            DataType::Bool => handle_type!(Bool, bool),
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
            DataType::Arrow3D => handle_type!(Arrow3D, re_log_types::Arrow3D),
            DataType::Tensor => handle_type!(Tensor, re_log_types::Tensor),
            DataType::ObjPath => handle_type!(ObjPath, ObjPath),
            DataType::Transform => handle_type!(Transform, re_log_types::Transform),
            DataType::DataVec => handle_type!(DataVec, DataVec),
        }
    }
}

// ----------------------------------------------------------------------------

/// Stores the history of a mono-field.
pub struct MonoFieldStore<Time, T> {
    pub(crate) history: BTreeMap<Time, (MsgId, T)>,
}

impl<Time, T> Default for MonoFieldStore<Time, T> {
    fn default() -> Self {
        Self {
            history: Default::default(),
        }
    }
}

impl<Time: 'static + Copy + Ord, T: DataTrait> MonoFieldStore<Time, T> {
    pub fn query<'slf>(
        &'slf self,
        time_query: &TimeQuery<Time>,
        mut visit: impl FnMut(&Time, &MsgId, &'slf T),
    ) {
        crate::query::query(&self.history, time_query, |time, (msg_id, value)| {
            visit(time, msg_id, value);
        });
    }

    /// Get the latest value (unless empty)
    pub fn latest(&self) -> Option<(&Time, &(MsgId, T))> {
        self.history.iter().rev().next()
    }
}

// ----------------------------------------------------------------------------

/// Stores the history of a multi-field.
pub(crate) struct MultiFieldStore<Time, T> {
    pub(crate) history: BTreeMap<Time, (MsgId, BatchOrSplat<T>)>,
}

impl<Time, T> Default for MultiFieldStore<Time, T> {
    fn default() -> Self {
        Self {
            history: Default::default(),
        }
    }
}

impl<Time: 'static + Copy + Ord, T: DataTrait> MultiFieldStore<Time, T> {
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
