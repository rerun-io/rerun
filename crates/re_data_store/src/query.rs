use std::collections::BTreeMap;

use re_log_types::{DataTrait, FieldName, IndexHash, MsgId};

use crate::*;

fn latest_at<'data, Time: Copy + Ord, T>(
    data_over_time: &'data BTreeMap<Time, T>,
    query_time: &'_ Time,
) -> Option<(&'data Time, &'data T)> {
    data_over_time.range(..=query_time).rev().next()
}

struct OnePastEnd<'d, Time, T, I>
where
    Time: 'static + Copy + Ord,
    T: 'static,
    I: Iterator<Item = (&'d Time, &'d T)>,
{
    start_time: Time,
    has_passed_start: bool,
    iter: I,
}

impl<'d, Time, I, T> Iterator for OnePastEnd<'d, Time, T, I>
where
    Time: 'static + Copy + Ord,
    T: 'static,
    I: Iterator<Item = (&'d Time, &'d T)>,
{
    type Item = (&'d Time, &'d T);

    fn next(&mut self) -> Option<Self::Item> {
        if self.has_passed_start {
            None
        } else {
            let (time, val) = self.iter.next()?;
            self.has_passed_start = *time <= self.start_time;
            Some((time, val))
        }
    }
}

fn values_in_range<'data, Time: 'static + Copy + Ord, T: 'static>(
    data_over_time: &'data BTreeMap<Time, T>,
    time_range: &'_ std::ops::RangeInclusive<Time>,
) -> impl Iterator<Item = (&'data Time, &'data T)> {
    let iter = data_over_time.range(..=time_range.end()).rev();
    OnePastEnd {
        iter,
        start_time: *time_range.start(),
        has_passed_start: false,
    }
}

#[test]
fn test_values_in_range() {
    use itertools::Itertools as _;
    let map = BTreeMap::from([(0, 0.0), (10, 10.0), (20, 20.0), (30, 30.0), (40, 40.0)]);

    let q = |range| {
        let mut v = values_in_range(&map, &range).collect_vec();
        v.sort_by_key(|(time, _)| *time);
        v
    };

    assert_eq!(q(10..=30), vec![(&10, &10.0), (&20, &20.0), (&30, &30.0)]);
    assert_eq!(q(15..=35), vec![(&10, &10.0), (&20, &20.0), (&30, &30.0)]);
}

pub(crate) fn query<'data, Time: 'static + Copy + Ord, T: 'static>(
    data_over_time: &'data BTreeMap<Time, T>,
    time_query: &TimeQuery<Time>,
    mut visit: impl FnMut(&Time, &'data T),
) {
    match time_query {
        TimeQuery::LatestAt(query_time) => {
            if let Some((_data_time, data)) = latest_at(data_over_time, query_time) {
                // we use `query_time` here instead of `data_time`
                // because we want to also query for the latest color, not the latest color at the time of the position.
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

pub(crate) struct MonoDataReader<'store, Time, T> {
    history: Option<&'store MonoFieldStore<Time, T>>,
}

impl<'store, Time: 'static + Copy + Ord, T: DataTrait> MonoDataReader<'store, Time, T> {
    pub fn new(store: &'store ObjStore<Time>, field_name: &FieldName) -> Self {
        Self {
            history: store.get_mono(field_name),
        }
    }

    pub fn latest_at(&self, query_time: &Time) -> Option<&'store T> {
        latest_at(&self.history?.history, query_time).map(|(_time, (_msg_id, value))| value)
    }
}

// ----------------------------------------------------------------------------

pub(crate) enum MultiDataReader<'store, T> {
    None,
    Splat(&'store T),
    Batch(&'store Batch<T>),
}

impl<'store, T: DataTrait> MultiDataReader<'store, T> {
    pub fn latest_at<Time: 'static + Copy + Ord>(
        history: Option<&'store MultiFieldStore<Time, T>>,
        query_time: &Time,
    ) -> Self {
        if let Some(history) = history {
            Self::latest_at_impl(history, query_time)
        } else {
            Self::None
        }
    }

    fn latest_at_impl<Time: 'static + Copy + Ord>(
        history: &'store MultiFieldStore<Time, T>,
        query_time: &Time,
    ) -> Self {
        if let Some((_, (_, batch))) = latest_at(&history.history, query_time) {
            match batch {
                BatchOrSplat::Splat(splat) => Self::Splat(splat),
                BatchOrSplat::Batch(batch) => Self::Batch(batch),
            }
        } else {
            Self::None
        }
    }

    pub fn get(&self, index: &IndexHash) -> Option<&'store T> {
        match self {
            Self::None => None,
            Self::Splat(splat) => Some(splat),
            Self::Batch(batch) => batch.get(index),
        }
    }
}

// ----------------------------------------------------------------------------

fn get_primary_batch<'a, T>(
    field_name: &'_ FieldName,
    batch_or_splat: &'a BatchOrSplat<T>,
) -> Option<&'a Batch<T>> {
    match batch_or_splat {
        BatchOrSplat::Splat(_) => {
            re_log::error!("Primary field {field_name:?} was a batch-splat.");
            None
        }
        BatchOrSplat::Batch(batch) => Some(batch),
    }
}

// ----------------------------------------------------------------------------

pub(crate) trait ChildTupleMappable<'store, Time> {
    type Keys;
    type Fields;
    type MonoReaders;
    type OptionMultiFieldStores;
    type MultiReaders;
    type Results;

    fn get_fields(keys: Self::Keys) -> Self::Fields;
    fn get_mono_readers(
        fields: Self::Fields,
        obj_store: &'store ObjStore<Time>,
    ) -> Self::MonoReaders;
    fn get_mono_results(mono_readers: &Self::MonoReaders, time: &Time) -> Self::Results;
    fn get_field_stores(
        fields: Self::Fields,
        obj_store: &'store ObjStore<Time>,
    ) -> Self::OptionMultiFieldStores;
    fn get_multi_readers(
        field_stores: &Self::OptionMultiFieldStores,
        time: &Time,
    ) -> Self::MultiReaders;
    fn get_multi_instance(
        multi_readers: &Self::MultiReaders,
        instance_index: &IndexHash,
    ) -> Self::Results;
}

#[impl_trait_for_tuples::impl_for_tuples(5)]
#[tuple_types_no_default_trait_bound]
impl<'store, Time: 'static + Copy + Ord> ChildTupleMappable<'store, Time> for TupleElem {
    for_tuples!(where #(TupleElem : DataTrait)* );
    for_tuples!(type Keys = ( #(&'static str),*); );
    for_tuples!(type Fields = ( #(FieldName),*); );
    for_tuples!(type MonoReaders = ( #(MonoDataReader<'store, Time, TupleElem>),*); );
    for_tuples!(type OptionMultiFieldStores = ( #(Option<&'store MultiFieldStore<Time, TupleElem>>),*); );
    for_tuples!(type MultiReaders = ( #(MultiDataReader<'store, TupleElem>),*); );
    for_tuples!(type Results = ( #(Option<&'store TupleElem>),*); );

    fn get_fields(keys: Self::Keys) -> Self::Fields {
        // (FieldName::from(child.0), ...)
        for_tuples!((#(FieldName::from(keys.TupleElem)),*));
    }

    fn get_mono_readers(
        fields: Self::Fields,
        obj_store: &'store ObjStore<Time>,
    ) -> Self::MonoReaders {
        // (MonoDataReader<Time, S1>::new(obj_store, &fields.0, ...)
        for_tuples!((#(MonoDataReader::<Time, TupleElem>::new(obj_store, &fields.TupleElem)),*));
    }

    fn get_mono_results(mono_readers: &Self::MonoReaders, time: &Time) -> Self::Results {
        // (mono_readers.0.latest_at(time), ...)
        for_tuples!((#(mono_readers.TupleElem.latest_at(time)),*));
    }

    fn get_field_stores(
        fields: Self::Fields,
        obj_store: &'store ObjStore<Time>,
    ) -> Self::OptionMultiFieldStores {
        // (mono_readers.0.latest_at(time), ...)
        for_tuples!((#(obj_store.get_multi::<TupleElem>(&fields.TupleElem)),*));
    }

    fn get_multi_readers(
        field_stores: &Self::OptionMultiFieldStores,
        time: &Time,
    ) -> Self::MultiReaders {
        // (MultiDataReader::latest_at(field_stores.0, time), ...)
        for_tuples!((#(MultiDataReader::latest_at(field_stores.TupleElem, time)),*));
    }

    fn get_multi_instance(
        multi_readers: &Self::MultiReaders,
        instance_index: &IndexHash,
    ) -> Self::Results {
        // (multi_readers.0.get(indstance_index), ...)
        for_tuples!((#(multi_readers.TupleElem.get(instance_index)),*));
    }
}

pub(crate) fn visit_type_data_n<
    's,
    Time: 'static + Copy + Ord,
    T: DataTrait,
    ChildTuple: ChildTupleMappable<'s, Time>,
>(
    obj_store: &'s ObjStore<Time>,
    field_name: &FieldName,
    time_query: &TimeQuery<Time>,
    child_keys: ChildTuple::Keys,
    mut visit: impl FnMut(Option<&'s IndexHash>, Time, &'s MsgId, &'s T, ChildTuple::Results),
) -> Option<()> {
    crate::profile_function!();

    let child_fields = ChildTuple::get_fields(child_keys);

    if obj_store.mono() {
        let primary = obj_store.get_mono::<T>(field_name)?;
        let child_mono_readers = ChildTuple::get_mono_readers(child_fields, obj_store);
        query(
            &primary.history,
            time_query,
            |time, (msg_id, primary_value)| {
                let results = ChildTuple::get_mono_results(&child_mono_readers, time);
                visit(None, *time, msg_id, primary_value, results);
            },
        );
    } else {
        let primary = obj_store.get_multi::<T>(field_name)?;
        let child_field_stores = ChildTuple::get_field_stores(child_fields, obj_store);
        query(
            &primary.history,
            time_query,
            |time, (msg_id, primary_batch)| {
                if let Some(primary_batch) = get_primary_batch(field_name, primary_batch) {
                    let multi_readers = ChildTuple::get_multi_readers(&child_field_stores, time);
                    for (instance_index, primary_value) in primary_batch.iter() {
                        let results =
                            ChildTuple::get_multi_instance(&multi_readers, instance_index);
                        visit(Some(instance_index), *time, msg_id, primary_value, results);
                    }
                }
            },
        );
    }

    Some(())
}
