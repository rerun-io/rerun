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

struct MonoDataReader<'store, Time, T> {
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

enum MultiDataReader<'store, T> {
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

/// Query all objects of the same type path (but different index paths).
pub fn visit_type_data<'s, Time: 'static + Copy + Ord, T: DataTrait>(
    obj_store: &'s ObjStore<Time>,
    field_name: &FieldName,
    time_query: &TimeQuery<Time>,
    mut visit: impl FnMut(Option<&'s IndexHash>, &'s MsgId, &'s T),
) -> Option<()> {
    crate::profile_function!();

    if obj_store.mono() {
        let primary = obj_store.get_mono::<T>(field_name)?;
        query(
            &primary.history,
            time_query,
            |_time, (msg_id, primary_value)| {
                visit(None, msg_id, primary_value);
            },
        );
    } else {
        let primary = obj_store.get_multi::<T>(field_name)?;
        query(
            &primary.history,
            time_query,
            |_time, (msg_id, primary_batch)| {
                if let Some(primary_batch) = get_primary_batch(field_name, primary_batch) {
                    for (instance_index, primary_value) in primary_batch.iter() {
                        visit(Some(instance_index), msg_id, primary_value);
                    }
                }
            },
        );
    }

    Some(())
}

pub fn visit_type_data_1<'s, Time: 'static + Copy + Ord, T: DataTrait, S1: DataTrait>(
    obj_store: &'s ObjStore<Time>,
    field_name: &FieldName,
    time_query: &TimeQuery<Time>,
    (child1,): (&str,),
    mut visit: impl FnMut(Option<&'s IndexHash>, Time, &'s MsgId, &'s T, Option<&'s S1>),
) -> Option<()> {
    crate::profile_function!();
    let child1 = FieldName::from(child1);

    if obj_store.mono() {
        let primary = obj_store.get_mono::<T>(field_name)?;
        let child1 = MonoDataReader::<Time, S1>::new(obj_store, &child1);
        query(
            &primary.history,
            time_query,
            |time, (msg_id, primary_value)| {
                visit(None, *time, msg_id, primary_value, child1.latest_at(time));
            },
        );
    } else {
        let primary = obj_store.get_multi::<T>(field_name)?;
        let child1 = obj_store.get_multi::<S1>(&child1);
        query(
            &primary.history,
            time_query,
            |time, (msg_id, primary_batch)| {
                if let Some(primary_batch) = get_primary_batch(field_name, primary_batch) {
                    let child1 = MultiDataReader::latest_at(child1, time);
                    for (instance_index, primary_value) in primary_batch.iter() {
                        visit(
                            Some(instance_index),
                            *time,
                            msg_id,
                            primary_value,
                            child1.get(instance_index),
                        );
                    }
                }
            },
        );
    }

    Some(())
}

pub fn visit_type_data_2<
    's,
    Time: 'static + Copy + Ord,
    T: DataTrait,
    S1: DataTrait,
    S2: DataTrait,
>(
    obj_store: &'s ObjStore<Time>,
    field_name: &FieldName,
    time_query: &TimeQuery<Time>,
    (child1, child2): (&str, &str),
    mut visit: impl FnMut(Option<&'s IndexHash>, Time, &'s MsgId, &'s T, Option<&'s S1>, Option<&'s S2>),
) -> Option<()> {
    crate::profile_function!();
    let child1 = FieldName::from(child1);
    let child2 = FieldName::from(child2);

    if obj_store.mono() {
        let primary = obj_store.get_mono::<T>(field_name)?;
        let child1 = MonoDataReader::<Time, S1>::new(obj_store, &child1);
        let child2 = MonoDataReader::<Time, S2>::new(obj_store, &child2);
        query(
            &primary.history,
            time_query,
            |time, (msg_id, primary_value)| {
                visit(
                    None,
                    *time,
                    msg_id,
                    primary_value,
                    child1.latest_at(time),
                    child2.latest_at(time),
                );
            },
        );
    } else {
        let primary = obj_store.get_multi::<T>(field_name)?;
        let child1 = obj_store.get_multi::<S1>(&child1);
        let child2 = obj_store.get_multi::<S2>(&child2);
        query(
            &primary.history,
            time_query,
            |time, (msg_id, primary_batch)| {
                if let Some(primary_batch) = get_primary_batch(field_name, primary_batch) {
                    let child1 = MultiDataReader::latest_at(child1, time);
                    let child2 = MultiDataReader::latest_at(child2, time);
                    for (instance_index, primary_value) in primary_batch.iter() {
                        visit(
                            Some(instance_index),
                            *time,
                            msg_id,
                            primary_value,
                            child1.get(instance_index),
                            child2.get(instance_index),
                        );
                    }
                }
            },
        );
    }

    Some(())
}

pub fn visit_type_data_3<
    's,
    Time: 'static + Copy + Ord,
    T: DataTrait,
    S1: DataTrait,
    S2: DataTrait,
    S3: DataTrait,
>(
    obj_store: &'s ObjStore<Time>,
    field_name: &FieldName,
    time_query: &TimeQuery<Time>,
    (child1, child2, child3): (&str, &str, &str),
    mut visit: impl FnMut(
        Option<&'s IndexHash>,
        Time,
        &'s MsgId,
        &'s T,
        Option<&'s S1>,
        Option<&'s S2>,
        Option<&'s S3>,
    ),
) -> Option<()> {
    crate::profile_function!();
    let child1 = FieldName::from(child1);
    let child2 = FieldName::from(child2);
    let child3 = FieldName::from(child3);

    if obj_store.mono() {
        let primary = obj_store.get_mono::<T>(field_name)?;
        let child1 = MonoDataReader::<Time, S1>::new(obj_store, &child1);
        let child2 = MonoDataReader::<Time, S2>::new(obj_store, &child2);
        let child3 = MonoDataReader::<Time, S3>::new(obj_store, &child3);
        query(
            &primary.history,
            time_query,
            |time, (msg_id, primary_value)| {
                visit(
                    None,
                    *time,
                    msg_id,
                    primary_value,
                    child1.latest_at(time),
                    child2.latest_at(time),
                    child3.latest_at(time),
                );
            },
        );
    } else {
        let primary = obj_store.get_multi::<T>(field_name)?;
        let child1 = obj_store.get_multi::<S1>(&child1);
        let child2 = obj_store.get_multi::<S2>(&child2);
        let child3 = obj_store.get_multi::<S3>(&child3);
        query(
            &primary.history,
            time_query,
            |time, (msg_id, primary_batch)| {
                if let Some(primary_batch) = get_primary_batch(field_name, primary_batch) {
                    let child1 = MultiDataReader::latest_at(child1, time);
                    let child2 = MultiDataReader::latest_at(child2, time);
                    let child3 = MultiDataReader::latest_at(child3, time);
                    for (instance_index, primary_value) in primary_batch.iter() {
                        visit(
                            Some(instance_index),
                            *time,
                            msg_id,
                            primary_value,
                            child1.get(instance_index),
                            child2.get(instance_index),
                            child3.get(instance_index),
                        );
                    }
                }
            },
        );
    }

    Some(())
}

pub fn visit_type_data_4<
    's,
    Time: 'static + Copy + Ord,
    T: DataTrait,
    S1: DataTrait,
    S2: DataTrait,
    S3: DataTrait,
    S4: DataTrait,
>(
    obj_store: &'s ObjStore<Time>,
    field_name: &FieldName,
    time_query: &TimeQuery<Time>,
    (child1, child2, child3, child4): (&str, &str, &str, &str),
    mut visit: impl FnMut(
        Option<&'s IndexHash>,
        Time,
        &'s MsgId,
        &'s T,
        Option<&'s S1>,
        Option<&'s S2>,
        Option<&'s S3>,
        Option<&'s S4>,
    ),
) -> Option<()> {
    crate::profile_function!();
    let child1 = FieldName::from(child1);
    let child2 = FieldName::from(child2);
    let child3 = FieldName::from(child3);
    let child4 = FieldName::from(child4);

    if obj_store.mono() {
        let primary = obj_store.get_mono::<T>(field_name)?;
        let child1 = MonoDataReader::<Time, S1>::new(obj_store, &child1);
        let child2 = MonoDataReader::<Time, S2>::new(obj_store, &child2);
        let child3 = MonoDataReader::<Time, S3>::new(obj_store, &child3);
        let child4 = MonoDataReader::<Time, S4>::new(obj_store, &child4);
        query(
            &primary.history,
            time_query,
            |time, (msg_id, primary_value)| {
                visit(
                    None,
                    *time,
                    msg_id,
                    primary_value,
                    child1.latest_at(time),
                    child2.latest_at(time),
                    child3.latest_at(time),
                    child4.latest_at(time),
                );
            },
        );
    } else {
        let primary = obj_store.get_multi::<T>(field_name)?;
        let child1 = obj_store.get_multi::<S1>(&child1);
        let child2 = obj_store.get_multi::<S2>(&child2);
        let child3 = obj_store.get_multi::<S3>(&child3);
        let child4 = obj_store.get_multi::<S4>(&child4);
        query(
            &primary.history,
            time_query,
            |time, (msg_id, primary_batch)| {
                if let Some(primary_batch) = get_primary_batch(field_name, primary_batch) {
                    let child1 = MultiDataReader::latest_at(child1, time);
                    let child2 = MultiDataReader::latest_at(child2, time);
                    let child3 = MultiDataReader::latest_at(child3, time);
                    let child4 = MultiDataReader::latest_at(child4, time);
                    for (instance_index, primary_value) in primary_batch.iter() {
                        visit(
                            Some(instance_index),
                            *time,
                            msg_id,
                            primary_value,
                            child1.get(instance_index),
                            child2.get(instance_index),
                            child3.get(instance_index),
                            child4.get(instance_index),
                        );
                    }
                }
            },
        );
    }

    Some(())
}
