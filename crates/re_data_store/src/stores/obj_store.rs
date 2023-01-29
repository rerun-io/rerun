use nohash_hasher::IntMap;
use re_log_types::{DataTrait, FieldName, MsgId};

use crate::{BatchOrSplat, Error, FieldStore, MonoFieldStore, MultiFieldStore, Result};

// ----------------------------------------------------------------------------

/// Stores all the fields of a specific [`re_log_types::ObjPath`] on a specific [`re_log_types::Timeline`].
pub struct ObjStore<Time> {
    mono: bool,
    fields: IntMap<FieldName, FieldStore<Time>>,
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
    #[inline]
    pub fn mono(&self) -> bool {
        self.mono
    }

    #[inline]
    pub fn iter(&self) -> impl ExactSizeIterator<Item = (&FieldName, &FieldStore<Time>)> {
        self.fields.iter()
    }

    #[inline]
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

    pub fn insert_mono<T: DataTrait>(
        &mut self,
        field_name: FieldName,
        time: Time,
        msg_id: MsgId,
        value: Option<T>,
    ) -> Result<()> {
        if self.fields.is_empty() {
            self.mono = true; // first insertion - we can decide that we are mono from now on
        } else if !self.mono {
            return Err(Error::MixingMonoAndMulti);
        }

        let mono = self
            .fields
            .entry(field_name)
            .or_insert_with(|| FieldStore::new_mono::<T>())
            .get_mono_mut::<T>()?;

        mono.history.insert((time, msg_id), value);
        Ok(())
    }

    pub fn insert_batch<T: DataTrait>(
        &mut self,
        field_name: FieldName,
        time: Time,
        msg_id: MsgId,
        batch: BatchOrSplat<T>,
    ) -> Result<()> {
        if self.fields.is_empty() {
            self.mono = false; // first insertion - we can decide that we are multi from now on
        } else if self.mono {
            return Err(Error::MixingMonoAndMulti);
        }

        let multi = self
            .fields
            .entry(field_name)
            .or_insert_with(|| FieldStore::new_multi::<T>())
            .get_multi_mut::<T>()?;

        multi.history.insert((time, msg_id), batch);

        Ok(())
    }

    pub fn purge(&mut self, drop_msg_ids: &ahash::HashSet<MsgId>) {
        let Self { mono: _, fields } = self;
        for field_store in fields.values_mut() {
            field_store.purge(drop_msg_ids);
        }
    }
}

// ----------------------------------------------------------------------------

#[test]
fn test_obj_store() {
    use re_log_types::DataType;

    let mut obj_store = ObjStore::default();

    assert!(obj_store
        .insert_mono("field1".into(), 0, MsgId::random(), Some(3.15_f32))
        .is_ok());

    assert!(obj_store
        .insert_mono("field1".into(), 0, MsgId::random(), Some(3.15_f32))
        .is_ok());

    assert!(obj_store
        .insert_mono("field2".into(), 0, MsgId::random(), Some(3.15_f32))
        .is_ok());

    assert!(matches!(
        obj_store.insert_mono("field2".into(), 0, MsgId::random(), Some(42)),
        Err(crate::Error::MixingTypes {
            existing: DataType::F32,
            expected: DataType::I32
        })
    ));

    let batch = crate::BatchOrSplat::Splat(42.0);
    assert!(matches!(
        obj_store.insert_batch("field3".into(), 0, MsgId::random(), batch),
        Err(crate::Error::MixingMonoAndMulti)
    ));
}
