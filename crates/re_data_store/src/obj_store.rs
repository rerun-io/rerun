use nohash_hasher::IntMap;
use re_log_types::{DataTrait, FieldName, MsgId};

use crate::{BatchOrSplat, Error, FieldStore, MonoFieldStore, MultiFieldStore, Result};

// ----------------------------------------------------------------------------

/// Stores all the fields of a specific [`re_log_types::ObjPath`] on a specific [`re_log_types::TimeSource`].
pub struct ObjStore<Time> {
    pub(crate) mono: bool,
    pub(crate) fields: IntMap<FieldName, FieldStore<Time>>,
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

    pub fn iter(&self) -> impl ExactSizeIterator<Item = (&FieldName, &FieldStore<Time>)> {
        self.fields.iter()
    }

    pub fn get(&self, field_name: &FieldName) -> Option<&FieldStore<Time>> {
        self.fields.get(field_name)
    }

    pub(crate) fn get_mono<T: DataTrait>(
        &self,
        field_name: &FieldName,
    ) -> Option<&MonoFieldStore<Time, T>> {
        assert!(self.mono);
        self.fields.get(field_name)?.get_mono::<T>().ok()
    }

    pub(crate) fn get_multi<T: DataTrait>(
        &self,
        field_name: &FieldName,
    ) -> Option<&MultiFieldStore<Time, T>> {
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
            .or_insert_with(|| FieldStore::new_multi::<T>())
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
            .or_insert_with(|| FieldStore::new_mono::<T>())
            .get_mono_mut::<T>()?;

        mono.history.insert(time, (msg_id, value));
        Ok(())
    }
}

// ----------------------------------------------------------------------------

#[test]
fn test_obj_store() {
    use re_log_types::DataType;

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
