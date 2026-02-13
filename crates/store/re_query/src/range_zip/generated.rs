// This file was generated using `cargo r -p re_query --all-features --bin range_zip`.
// DO NOT EDIT.

// ---

#![expect(clippy::iter_on_single_items)]
#![expect(clippy::too_many_arguments)]
#![expect(clippy::type_complexity)]

use std::iter::Peekable;

/// Returns a new [`RangeZip1x1`] iterator.
///
/// The number of elements in a range zip iterator corresponds to the number of elements in the
/// shortest of its required iterators (`r0`).
///
/// Each call to `next` is guaranteed to yield the next value for each required iterator,
/// as well as the most recent index amongst all of them.
///
/// Optional iterators accumulate their state and yield their most recent value (if any),
/// each time the required iterators fire.
pub fn range_zip_1x1<Idx, IR0, R0, IO0, O0>(
    r0: IR0,
    o0: IO0,
) -> RangeZip1x1<Idx, IR0::IntoIter, R0, IO0::IntoIter, O0>
where
    Idx: std::cmp::Ord,
    IR0: IntoIterator<Item = (Idx, R0)>,
    IO0: IntoIterator<Item = (Idx, O0)>,
{
    RangeZip1x1 {
        r0: r0.into_iter(),
        o0: o0.into_iter().peekable(),

        o0_data_latest: None,
    }
}

/// Implements a range zip iterator combinator with 2 required iterators and 2 optional
/// iterators.
///
/// See [`range_zip_1x1`] for more information.
pub struct RangeZip1x1<Idx, IR0, R0, IO0, O0>
where
    Idx: std::cmp::Ord,
    IR0: Iterator<Item = (Idx, R0)>,
    IO0: Iterator<Item = (Idx, O0)>,
{
    r0: IR0,
    o0: Peekable<IO0>,

    o0_data_latest: Option<O0>,
}

impl<Idx, IR0, R0, IO0, O0> Iterator for RangeZip1x1<Idx, IR0, R0, IO0, O0>
where
    Idx: std::cmp::Ord,
    IR0: Iterator<Item = (Idx, R0)>,
    IO0: Iterator<Item = (Idx, O0)>,
    O0: Clone,
{
    type Item = (Idx, R0, Option<O0>);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let Self {
            r0,
            o0,
            o0_data_latest,
        } = self;

        let (r0_index, r0_data) = r0.next()?;

        let max_index = [r0_index].into_iter().max()?;

        let mut o0_data = None;
        while let Some((_, data)) = o0.next_if(|(index, _)| index <= &max_index) {
            o0_data = Some(data);
        }
        let o0_data = o0_data.or_else(|| o0_data_latest.take());
        o0_data_latest.clone_from(&o0_data);

        Some((max_index, r0_data, o0_data))
    }
}

/// Returns a new [`RangeZip1x2`] iterator.
///
/// The number of elements in a range zip iterator corresponds to the number of elements in the
/// shortest of its required iterators (`r0`).
///
/// Each call to `next` is guaranteed to yield the next value for each required iterator,
/// as well as the most recent index amongst all of them.
///
/// Optional iterators accumulate their state and yield their most recent value (if any),
/// each time the required iterators fire.
pub fn range_zip_1x2<Idx, IR0, R0, IO0, O0, IO1, O1>(
    r0: IR0,
    o0: IO0,
    o1: IO1,
) -> RangeZip1x2<Idx, IR0::IntoIter, R0, IO0::IntoIter, O0, IO1::IntoIter, O1>
where
    Idx: std::cmp::Ord,
    IR0: IntoIterator<Item = (Idx, R0)>,
    IO0: IntoIterator<Item = (Idx, O0)>,
    IO1: IntoIterator<Item = (Idx, O1)>,
{
    RangeZip1x2 {
        r0: r0.into_iter(),
        o0: o0.into_iter().peekable(),
        o1: o1.into_iter().peekable(),

        o0_data_latest: None,
        o1_data_latest: None,
    }
}

/// Implements a range zip iterator combinator with 2 required iterators and 2 optional
/// iterators.
///
/// See [`range_zip_1x2`] for more information.
pub struct RangeZip1x2<Idx, IR0, R0, IO0, O0, IO1, O1>
where
    Idx: std::cmp::Ord,
    IR0: Iterator<Item = (Idx, R0)>,
    IO0: Iterator<Item = (Idx, O0)>,
    IO1: Iterator<Item = (Idx, O1)>,
{
    r0: IR0,
    o0: Peekable<IO0>,
    o1: Peekable<IO1>,

    o0_data_latest: Option<O0>,
    o1_data_latest: Option<O1>,
}

impl<Idx, IR0, R0, IO0, O0, IO1, O1> Iterator for RangeZip1x2<Idx, IR0, R0, IO0, O0, IO1, O1>
where
    Idx: std::cmp::Ord,
    IR0: Iterator<Item = (Idx, R0)>,
    IO0: Iterator<Item = (Idx, O0)>,
    IO1: Iterator<Item = (Idx, O1)>,
    O0: Clone,
    O1: Clone,
{
    type Item = (Idx, R0, Option<O0>, Option<O1>);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let Self {
            r0,
            o0,
            o1,
            o0_data_latest,
            o1_data_latest,
        } = self;

        let (r0_index, r0_data) = r0.next()?;

        let max_index = [r0_index].into_iter().max()?;

        let mut o0_data = None;
        while let Some((_, data)) = o0.next_if(|(index, _)| index <= &max_index) {
            o0_data = Some(data);
        }
        let o0_data = o0_data.or_else(|| o0_data_latest.take());
        o0_data_latest.clone_from(&o0_data);

        let mut o1_data = None;
        while let Some((_, data)) = o1.next_if(|(index, _)| index <= &max_index) {
            o1_data = Some(data);
        }
        let o1_data = o1_data.or_else(|| o1_data_latest.take());
        o1_data_latest.clone_from(&o1_data);

        Some((max_index, r0_data, o0_data, o1_data))
    }
}

/// Returns a new [`RangeZip1x3`] iterator.
///
/// The number of elements in a range zip iterator corresponds to the number of elements in the
/// shortest of its required iterators (`r0`).
///
/// Each call to `next` is guaranteed to yield the next value for each required iterator,
/// as well as the most recent index amongst all of them.
///
/// Optional iterators accumulate their state and yield their most recent value (if any),
/// each time the required iterators fire.
pub fn range_zip_1x3<Idx, IR0, R0, IO0, O0, IO1, O1, IO2, O2>(
    r0: IR0,
    o0: IO0,
    o1: IO1,
    o2: IO2,
) -> RangeZip1x3<Idx, IR0::IntoIter, R0, IO0::IntoIter, O0, IO1::IntoIter, O1, IO2::IntoIter, O2>
where
    Idx: std::cmp::Ord,
    IR0: IntoIterator<Item = (Idx, R0)>,
    IO0: IntoIterator<Item = (Idx, O0)>,
    IO1: IntoIterator<Item = (Idx, O1)>,
    IO2: IntoIterator<Item = (Idx, O2)>,
{
    RangeZip1x3 {
        r0: r0.into_iter(),
        o0: o0.into_iter().peekable(),
        o1: o1.into_iter().peekable(),
        o2: o2.into_iter().peekable(),

        o0_data_latest: None,
        o1_data_latest: None,
        o2_data_latest: None,
    }
}

/// Implements a range zip iterator combinator with 2 required iterators and 2 optional
/// iterators.
///
/// See [`range_zip_1x3`] for more information.
pub struct RangeZip1x3<Idx, IR0, R0, IO0, O0, IO1, O1, IO2, O2>
where
    Idx: std::cmp::Ord,
    IR0: Iterator<Item = (Idx, R0)>,
    IO0: Iterator<Item = (Idx, O0)>,
    IO1: Iterator<Item = (Idx, O1)>,
    IO2: Iterator<Item = (Idx, O2)>,
{
    r0: IR0,
    o0: Peekable<IO0>,
    o1: Peekable<IO1>,
    o2: Peekable<IO2>,

    o0_data_latest: Option<O0>,
    o1_data_latest: Option<O1>,
    o2_data_latest: Option<O2>,
}

impl<Idx, IR0, R0, IO0, O0, IO1, O1, IO2, O2> Iterator
    for RangeZip1x3<Idx, IR0, R0, IO0, O0, IO1, O1, IO2, O2>
where
    Idx: std::cmp::Ord,
    IR0: Iterator<Item = (Idx, R0)>,
    IO0: Iterator<Item = (Idx, O0)>,
    IO1: Iterator<Item = (Idx, O1)>,
    IO2: Iterator<Item = (Idx, O2)>,
    O0: Clone,
    O1: Clone,
    O2: Clone,
{
    type Item = (Idx, R0, Option<O0>, Option<O1>, Option<O2>);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let Self {
            r0,
            o0,
            o1,
            o2,
            o0_data_latest,
            o1_data_latest,
            o2_data_latest,
        } = self;

        let (r0_index, r0_data) = r0.next()?;

        let max_index = [r0_index].into_iter().max()?;

        let mut o0_data = None;
        while let Some((_, data)) = o0.next_if(|(index, _)| index <= &max_index) {
            o0_data = Some(data);
        }
        let o0_data = o0_data.or_else(|| o0_data_latest.take());
        o0_data_latest.clone_from(&o0_data);

        let mut o1_data = None;
        while let Some((_, data)) = o1.next_if(|(index, _)| index <= &max_index) {
            o1_data = Some(data);
        }
        let o1_data = o1_data.or_else(|| o1_data_latest.take());
        o1_data_latest.clone_from(&o1_data);

        let mut o2_data = None;
        while let Some((_, data)) = o2.next_if(|(index, _)| index <= &max_index) {
            o2_data = Some(data);
        }
        let o2_data = o2_data.or_else(|| o2_data_latest.take());
        o2_data_latest.clone_from(&o2_data);

        Some((max_index, r0_data, o0_data, o1_data, o2_data))
    }
}

/// Returns a new [`RangeZip1x4`] iterator.
///
/// The number of elements in a range zip iterator corresponds to the number of elements in the
/// shortest of its required iterators (`r0`).
///
/// Each call to `next` is guaranteed to yield the next value for each required iterator,
/// as well as the most recent index amongst all of them.
///
/// Optional iterators accumulate their state and yield their most recent value (if any),
/// each time the required iterators fire.
pub fn range_zip_1x4<Idx, IR0, R0, IO0, O0, IO1, O1, IO2, O2, IO3, O3>(
    r0: IR0,
    o0: IO0,
    o1: IO1,
    o2: IO2,
    o3: IO3,
) -> RangeZip1x4<
    Idx,
    IR0::IntoIter,
    R0,
    IO0::IntoIter,
    O0,
    IO1::IntoIter,
    O1,
    IO2::IntoIter,
    O2,
    IO3::IntoIter,
    O3,
>
where
    Idx: std::cmp::Ord,
    IR0: IntoIterator<Item = (Idx, R0)>,
    IO0: IntoIterator<Item = (Idx, O0)>,
    IO1: IntoIterator<Item = (Idx, O1)>,
    IO2: IntoIterator<Item = (Idx, O2)>,
    IO3: IntoIterator<Item = (Idx, O3)>,
{
    RangeZip1x4 {
        r0: r0.into_iter(),
        o0: o0.into_iter().peekable(),
        o1: o1.into_iter().peekable(),
        o2: o2.into_iter().peekable(),
        o3: o3.into_iter().peekable(),

        o0_data_latest: None,
        o1_data_latest: None,
        o2_data_latest: None,
        o3_data_latest: None,
    }
}

/// Implements a range zip iterator combinator with 2 required iterators and 2 optional
/// iterators.
///
/// See [`range_zip_1x4`] for more information.
pub struct RangeZip1x4<Idx, IR0, R0, IO0, O0, IO1, O1, IO2, O2, IO3, O3>
where
    Idx: std::cmp::Ord,
    IR0: Iterator<Item = (Idx, R0)>,
    IO0: Iterator<Item = (Idx, O0)>,
    IO1: Iterator<Item = (Idx, O1)>,
    IO2: Iterator<Item = (Idx, O2)>,
    IO3: Iterator<Item = (Idx, O3)>,
{
    r0: IR0,
    o0: Peekable<IO0>,
    o1: Peekable<IO1>,
    o2: Peekable<IO2>,
    o3: Peekable<IO3>,

    o0_data_latest: Option<O0>,
    o1_data_latest: Option<O1>,
    o2_data_latest: Option<O2>,
    o3_data_latest: Option<O3>,
}

impl<Idx, IR0, R0, IO0, O0, IO1, O1, IO2, O2, IO3, O3> Iterator
    for RangeZip1x4<Idx, IR0, R0, IO0, O0, IO1, O1, IO2, O2, IO3, O3>
where
    Idx: std::cmp::Ord,
    IR0: Iterator<Item = (Idx, R0)>,
    IO0: Iterator<Item = (Idx, O0)>,
    IO1: Iterator<Item = (Idx, O1)>,
    IO2: Iterator<Item = (Idx, O2)>,
    IO3: Iterator<Item = (Idx, O3)>,
    O0: Clone,
    O1: Clone,
    O2: Clone,
    O3: Clone,
{
    type Item = (Idx, R0, Option<O0>, Option<O1>, Option<O2>, Option<O3>);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let Self {
            r0,
            o0,
            o1,
            o2,
            o3,
            o0_data_latest,
            o1_data_latest,
            o2_data_latest,
            o3_data_latest,
        } = self;

        let (r0_index, r0_data) = r0.next()?;

        let max_index = [r0_index].into_iter().max()?;

        let mut o0_data = None;
        while let Some((_, data)) = o0.next_if(|(index, _)| index <= &max_index) {
            o0_data = Some(data);
        }
        let o0_data = o0_data.or_else(|| o0_data_latest.take());
        o0_data_latest.clone_from(&o0_data);

        let mut o1_data = None;
        while let Some((_, data)) = o1.next_if(|(index, _)| index <= &max_index) {
            o1_data = Some(data);
        }
        let o1_data = o1_data.or_else(|| o1_data_latest.take());
        o1_data_latest.clone_from(&o1_data);

        let mut o2_data = None;
        while let Some((_, data)) = o2.next_if(|(index, _)| index <= &max_index) {
            o2_data = Some(data);
        }
        let o2_data = o2_data.or_else(|| o2_data_latest.take());
        o2_data_latest.clone_from(&o2_data);

        let mut o3_data = None;
        while let Some((_, data)) = o3.next_if(|(index, _)| index <= &max_index) {
            o3_data = Some(data);
        }
        let o3_data = o3_data.or_else(|| o3_data_latest.take());
        o3_data_latest.clone_from(&o3_data);

        Some((max_index, r0_data, o0_data, o1_data, o2_data, o3_data))
    }
}

/// Returns a new [`RangeZip1x5`] iterator.
///
/// The number of elements in a range zip iterator corresponds to the number of elements in the
/// shortest of its required iterators (`r0`).
///
/// Each call to `next` is guaranteed to yield the next value for each required iterator,
/// as well as the most recent index amongst all of them.
///
/// Optional iterators accumulate their state and yield their most recent value (if any),
/// each time the required iterators fire.
pub fn range_zip_1x5<Idx, IR0, R0, IO0, O0, IO1, O1, IO2, O2, IO3, O3, IO4, O4>(
    r0: IR0,
    o0: IO0,
    o1: IO1,
    o2: IO2,
    o3: IO3,
    o4: IO4,
) -> RangeZip1x5<
    Idx,
    IR0::IntoIter,
    R0,
    IO0::IntoIter,
    O0,
    IO1::IntoIter,
    O1,
    IO2::IntoIter,
    O2,
    IO3::IntoIter,
    O3,
    IO4::IntoIter,
    O4,
>
where
    Idx: std::cmp::Ord,
    IR0: IntoIterator<Item = (Idx, R0)>,
    IO0: IntoIterator<Item = (Idx, O0)>,
    IO1: IntoIterator<Item = (Idx, O1)>,
    IO2: IntoIterator<Item = (Idx, O2)>,
    IO3: IntoIterator<Item = (Idx, O3)>,
    IO4: IntoIterator<Item = (Idx, O4)>,
{
    RangeZip1x5 {
        r0: r0.into_iter(),
        o0: o0.into_iter().peekable(),
        o1: o1.into_iter().peekable(),
        o2: o2.into_iter().peekable(),
        o3: o3.into_iter().peekable(),
        o4: o4.into_iter().peekable(),

        o0_data_latest: None,
        o1_data_latest: None,
        o2_data_latest: None,
        o3_data_latest: None,
        o4_data_latest: None,
    }
}

/// Implements a range zip iterator combinator with 2 required iterators and 2 optional
/// iterators.
///
/// See [`range_zip_1x5`] for more information.
pub struct RangeZip1x5<Idx, IR0, R0, IO0, O0, IO1, O1, IO2, O2, IO3, O3, IO4, O4>
where
    Idx: std::cmp::Ord,
    IR0: Iterator<Item = (Idx, R0)>,
    IO0: Iterator<Item = (Idx, O0)>,
    IO1: Iterator<Item = (Idx, O1)>,
    IO2: Iterator<Item = (Idx, O2)>,
    IO3: Iterator<Item = (Idx, O3)>,
    IO4: Iterator<Item = (Idx, O4)>,
{
    r0: IR0,
    o0: Peekable<IO0>,
    o1: Peekable<IO1>,
    o2: Peekable<IO2>,
    o3: Peekable<IO3>,
    o4: Peekable<IO4>,

    o0_data_latest: Option<O0>,
    o1_data_latest: Option<O1>,
    o2_data_latest: Option<O2>,
    o3_data_latest: Option<O3>,
    o4_data_latest: Option<O4>,
}

impl<Idx, IR0, R0, IO0, O0, IO1, O1, IO2, O2, IO3, O3, IO4, O4> Iterator
    for RangeZip1x5<Idx, IR0, R0, IO0, O0, IO1, O1, IO2, O2, IO3, O3, IO4, O4>
where
    Idx: std::cmp::Ord,
    IR0: Iterator<Item = (Idx, R0)>,
    IO0: Iterator<Item = (Idx, O0)>,
    IO1: Iterator<Item = (Idx, O1)>,
    IO2: Iterator<Item = (Idx, O2)>,
    IO3: Iterator<Item = (Idx, O3)>,
    IO4: Iterator<Item = (Idx, O4)>,
    O0: Clone,
    O1: Clone,
    O2: Clone,
    O3: Clone,
    O4: Clone,
{
    type Item = (
        Idx,
        R0,
        Option<O0>,
        Option<O1>,
        Option<O2>,
        Option<O3>,
        Option<O4>,
    );

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let Self {
            r0,
            o0,
            o1,
            o2,
            o3,
            o4,
            o0_data_latest,
            o1_data_latest,
            o2_data_latest,
            o3_data_latest,
            o4_data_latest,
        } = self;

        let (r0_index, r0_data) = r0.next()?;

        let max_index = [r0_index].into_iter().max()?;

        let mut o0_data = None;
        while let Some((_, data)) = o0.next_if(|(index, _)| index <= &max_index) {
            o0_data = Some(data);
        }
        let o0_data = o0_data.or_else(|| o0_data_latest.take());
        o0_data_latest.clone_from(&o0_data);

        let mut o1_data = None;
        while let Some((_, data)) = o1.next_if(|(index, _)| index <= &max_index) {
            o1_data = Some(data);
        }
        let o1_data = o1_data.or_else(|| o1_data_latest.take());
        o1_data_latest.clone_from(&o1_data);

        let mut o2_data = None;
        while let Some((_, data)) = o2.next_if(|(index, _)| index <= &max_index) {
            o2_data = Some(data);
        }
        let o2_data = o2_data.or_else(|| o2_data_latest.take());
        o2_data_latest.clone_from(&o2_data);

        let mut o3_data = None;
        while let Some((_, data)) = o3.next_if(|(index, _)| index <= &max_index) {
            o3_data = Some(data);
        }
        let o3_data = o3_data.or_else(|| o3_data_latest.take());
        o3_data_latest.clone_from(&o3_data);

        let mut o4_data = None;
        while let Some((_, data)) = o4.next_if(|(index, _)| index <= &max_index) {
            o4_data = Some(data);
        }
        let o4_data = o4_data.or_else(|| o4_data_latest.take());
        o4_data_latest.clone_from(&o4_data);

        Some((
            max_index, r0_data, o0_data, o1_data, o2_data, o3_data, o4_data,
        ))
    }
}

/// Returns a new [`RangeZip1x6`] iterator.
///
/// The number of elements in a range zip iterator corresponds to the number of elements in the
/// shortest of its required iterators (`r0`).
///
/// Each call to `next` is guaranteed to yield the next value for each required iterator,
/// as well as the most recent index amongst all of them.
///
/// Optional iterators accumulate their state and yield their most recent value (if any),
/// each time the required iterators fire.
pub fn range_zip_1x6<Idx, IR0, R0, IO0, O0, IO1, O1, IO2, O2, IO3, O3, IO4, O4, IO5, O5>(
    r0: IR0,
    o0: IO0,
    o1: IO1,
    o2: IO2,
    o3: IO3,
    o4: IO4,
    o5: IO5,
) -> RangeZip1x6<
    Idx,
    IR0::IntoIter,
    R0,
    IO0::IntoIter,
    O0,
    IO1::IntoIter,
    O1,
    IO2::IntoIter,
    O2,
    IO3::IntoIter,
    O3,
    IO4::IntoIter,
    O4,
    IO5::IntoIter,
    O5,
>
where
    Idx: std::cmp::Ord,
    IR0: IntoIterator<Item = (Idx, R0)>,
    IO0: IntoIterator<Item = (Idx, O0)>,
    IO1: IntoIterator<Item = (Idx, O1)>,
    IO2: IntoIterator<Item = (Idx, O2)>,
    IO3: IntoIterator<Item = (Idx, O3)>,
    IO4: IntoIterator<Item = (Idx, O4)>,
    IO5: IntoIterator<Item = (Idx, O5)>,
{
    RangeZip1x6 {
        r0: r0.into_iter(),
        o0: o0.into_iter().peekable(),
        o1: o1.into_iter().peekable(),
        o2: o2.into_iter().peekable(),
        o3: o3.into_iter().peekable(),
        o4: o4.into_iter().peekable(),
        o5: o5.into_iter().peekable(),

        o0_data_latest: None,
        o1_data_latest: None,
        o2_data_latest: None,
        o3_data_latest: None,
        o4_data_latest: None,
        o5_data_latest: None,
    }
}

/// Implements a range zip iterator combinator with 2 required iterators and 2 optional
/// iterators.
///
/// See [`range_zip_1x6`] for more information.
pub struct RangeZip1x6<Idx, IR0, R0, IO0, O0, IO1, O1, IO2, O2, IO3, O3, IO4, O4, IO5, O5>
where
    Idx: std::cmp::Ord,
    IR0: Iterator<Item = (Idx, R0)>,
    IO0: Iterator<Item = (Idx, O0)>,
    IO1: Iterator<Item = (Idx, O1)>,
    IO2: Iterator<Item = (Idx, O2)>,
    IO3: Iterator<Item = (Idx, O3)>,
    IO4: Iterator<Item = (Idx, O4)>,
    IO5: Iterator<Item = (Idx, O5)>,
{
    r0: IR0,
    o0: Peekable<IO0>,
    o1: Peekable<IO1>,
    o2: Peekable<IO2>,
    o3: Peekable<IO3>,
    o4: Peekable<IO4>,
    o5: Peekable<IO5>,

    o0_data_latest: Option<O0>,
    o1_data_latest: Option<O1>,
    o2_data_latest: Option<O2>,
    o3_data_latest: Option<O3>,
    o4_data_latest: Option<O4>,
    o5_data_latest: Option<O5>,
}

impl<Idx, IR0, R0, IO0, O0, IO1, O1, IO2, O2, IO3, O3, IO4, O4, IO5, O5> Iterator
    for RangeZip1x6<Idx, IR0, R0, IO0, O0, IO1, O1, IO2, O2, IO3, O3, IO4, O4, IO5, O5>
where
    Idx: std::cmp::Ord,
    IR0: Iterator<Item = (Idx, R0)>,
    IO0: Iterator<Item = (Idx, O0)>,
    IO1: Iterator<Item = (Idx, O1)>,
    IO2: Iterator<Item = (Idx, O2)>,
    IO3: Iterator<Item = (Idx, O3)>,
    IO4: Iterator<Item = (Idx, O4)>,
    IO5: Iterator<Item = (Idx, O5)>,
    O0: Clone,
    O1: Clone,
    O2: Clone,
    O3: Clone,
    O4: Clone,
    O5: Clone,
{
    type Item = (
        Idx,
        R0,
        Option<O0>,
        Option<O1>,
        Option<O2>,
        Option<O3>,
        Option<O4>,
        Option<O5>,
    );

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let Self {
            r0,
            o0,
            o1,
            o2,
            o3,
            o4,
            o5,
            o0_data_latest,
            o1_data_latest,
            o2_data_latest,
            o3_data_latest,
            o4_data_latest,
            o5_data_latest,
        } = self;

        let (r0_index, r0_data) = r0.next()?;

        let max_index = [r0_index].into_iter().max()?;

        let mut o0_data = None;
        while let Some((_, data)) = o0.next_if(|(index, _)| index <= &max_index) {
            o0_data = Some(data);
        }
        let o0_data = o0_data.or_else(|| o0_data_latest.take());
        o0_data_latest.clone_from(&o0_data);

        let mut o1_data = None;
        while let Some((_, data)) = o1.next_if(|(index, _)| index <= &max_index) {
            o1_data = Some(data);
        }
        let o1_data = o1_data.or_else(|| o1_data_latest.take());
        o1_data_latest.clone_from(&o1_data);

        let mut o2_data = None;
        while let Some((_, data)) = o2.next_if(|(index, _)| index <= &max_index) {
            o2_data = Some(data);
        }
        let o2_data = o2_data.or_else(|| o2_data_latest.take());
        o2_data_latest.clone_from(&o2_data);

        let mut o3_data = None;
        while let Some((_, data)) = o3.next_if(|(index, _)| index <= &max_index) {
            o3_data = Some(data);
        }
        let o3_data = o3_data.or_else(|| o3_data_latest.take());
        o3_data_latest.clone_from(&o3_data);

        let mut o4_data = None;
        while let Some((_, data)) = o4.next_if(|(index, _)| index <= &max_index) {
            o4_data = Some(data);
        }
        let o4_data = o4_data.or_else(|| o4_data_latest.take());
        o4_data_latest.clone_from(&o4_data);

        let mut o5_data = None;
        while let Some((_, data)) = o5.next_if(|(index, _)| index <= &max_index) {
            o5_data = Some(data);
        }
        let o5_data = o5_data.or_else(|| o5_data_latest.take());
        o5_data_latest.clone_from(&o5_data);

        Some((
            max_index, r0_data, o0_data, o1_data, o2_data, o3_data, o4_data, o5_data,
        ))
    }
}

/// Returns a new [`RangeZip1x7`] iterator.
///
/// The number of elements in a range zip iterator corresponds to the number of elements in the
/// shortest of its required iterators (`r0`).
///
/// Each call to `next` is guaranteed to yield the next value for each required iterator,
/// as well as the most recent index amongst all of them.
///
/// Optional iterators accumulate their state and yield their most recent value (if any),
/// each time the required iterators fire.
pub fn range_zip_1x7<Idx, IR0, R0, IO0, O0, IO1, O1, IO2, O2, IO3, O3, IO4, O4, IO5, O5, IO6, O6>(
    r0: IR0,
    o0: IO0,
    o1: IO1,
    o2: IO2,
    o3: IO3,
    o4: IO4,
    o5: IO5,
    o6: IO6,
) -> RangeZip1x7<
    Idx,
    IR0::IntoIter,
    R0,
    IO0::IntoIter,
    O0,
    IO1::IntoIter,
    O1,
    IO2::IntoIter,
    O2,
    IO3::IntoIter,
    O3,
    IO4::IntoIter,
    O4,
    IO5::IntoIter,
    O5,
    IO6::IntoIter,
    O6,
>
where
    Idx: std::cmp::Ord,
    IR0: IntoIterator<Item = (Idx, R0)>,
    IO0: IntoIterator<Item = (Idx, O0)>,
    IO1: IntoIterator<Item = (Idx, O1)>,
    IO2: IntoIterator<Item = (Idx, O2)>,
    IO3: IntoIterator<Item = (Idx, O3)>,
    IO4: IntoIterator<Item = (Idx, O4)>,
    IO5: IntoIterator<Item = (Idx, O5)>,
    IO6: IntoIterator<Item = (Idx, O6)>,
{
    RangeZip1x7 {
        r0: r0.into_iter(),
        o0: o0.into_iter().peekable(),
        o1: o1.into_iter().peekable(),
        o2: o2.into_iter().peekable(),
        o3: o3.into_iter().peekable(),
        o4: o4.into_iter().peekable(),
        o5: o5.into_iter().peekable(),
        o6: o6.into_iter().peekable(),

        o0_data_latest: None,
        o1_data_latest: None,
        o2_data_latest: None,
        o3_data_latest: None,
        o4_data_latest: None,
        o5_data_latest: None,
        o6_data_latest: None,
    }
}

/// Implements a range zip iterator combinator with 2 required iterators and 2 optional
/// iterators.
///
/// See [`range_zip_1x7`] for more information.
pub struct RangeZip1x7<Idx, IR0, R0, IO0, O0, IO1, O1, IO2, O2, IO3, O3, IO4, O4, IO5, O5, IO6, O6>
where
    Idx: std::cmp::Ord,
    IR0: Iterator<Item = (Idx, R0)>,
    IO0: Iterator<Item = (Idx, O0)>,
    IO1: Iterator<Item = (Idx, O1)>,
    IO2: Iterator<Item = (Idx, O2)>,
    IO3: Iterator<Item = (Idx, O3)>,
    IO4: Iterator<Item = (Idx, O4)>,
    IO5: Iterator<Item = (Idx, O5)>,
    IO6: Iterator<Item = (Idx, O6)>,
{
    r0: IR0,
    o0: Peekable<IO0>,
    o1: Peekable<IO1>,
    o2: Peekable<IO2>,
    o3: Peekable<IO3>,
    o4: Peekable<IO4>,
    o5: Peekable<IO5>,
    o6: Peekable<IO6>,

    o0_data_latest: Option<O0>,
    o1_data_latest: Option<O1>,
    o2_data_latest: Option<O2>,
    o3_data_latest: Option<O3>,
    o4_data_latest: Option<O4>,
    o5_data_latest: Option<O5>,
    o6_data_latest: Option<O6>,
}

impl<Idx, IR0, R0, IO0, O0, IO1, O1, IO2, O2, IO3, O3, IO4, O4, IO5, O5, IO6, O6> Iterator
    for RangeZip1x7<Idx, IR0, R0, IO0, O0, IO1, O1, IO2, O2, IO3, O3, IO4, O4, IO5, O5, IO6, O6>
where
    Idx: std::cmp::Ord,
    IR0: Iterator<Item = (Idx, R0)>,
    IO0: Iterator<Item = (Idx, O0)>,
    IO1: Iterator<Item = (Idx, O1)>,
    IO2: Iterator<Item = (Idx, O2)>,
    IO3: Iterator<Item = (Idx, O3)>,
    IO4: Iterator<Item = (Idx, O4)>,
    IO5: Iterator<Item = (Idx, O5)>,
    IO6: Iterator<Item = (Idx, O6)>,
    O0: Clone,
    O1: Clone,
    O2: Clone,
    O3: Clone,
    O4: Clone,
    O5: Clone,
    O6: Clone,
{
    type Item = (
        Idx,
        R0,
        Option<O0>,
        Option<O1>,
        Option<O2>,
        Option<O3>,
        Option<O4>,
        Option<O5>,
        Option<O6>,
    );

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let Self {
            r0,
            o0,
            o1,
            o2,
            o3,
            o4,
            o5,
            o6,
            o0_data_latest,
            o1_data_latest,
            o2_data_latest,
            o3_data_latest,
            o4_data_latest,
            o5_data_latest,
            o6_data_latest,
        } = self;

        let (r0_index, r0_data) = r0.next()?;

        let max_index = [r0_index].into_iter().max()?;

        let mut o0_data = None;
        while let Some((_, data)) = o0.next_if(|(index, _)| index <= &max_index) {
            o0_data = Some(data);
        }
        let o0_data = o0_data.or_else(|| o0_data_latest.take());
        o0_data_latest.clone_from(&o0_data);

        let mut o1_data = None;
        while let Some((_, data)) = o1.next_if(|(index, _)| index <= &max_index) {
            o1_data = Some(data);
        }
        let o1_data = o1_data.or_else(|| o1_data_latest.take());
        o1_data_latest.clone_from(&o1_data);

        let mut o2_data = None;
        while let Some((_, data)) = o2.next_if(|(index, _)| index <= &max_index) {
            o2_data = Some(data);
        }
        let o2_data = o2_data.or_else(|| o2_data_latest.take());
        o2_data_latest.clone_from(&o2_data);

        let mut o3_data = None;
        while let Some((_, data)) = o3.next_if(|(index, _)| index <= &max_index) {
            o3_data = Some(data);
        }
        let o3_data = o3_data.or_else(|| o3_data_latest.take());
        o3_data_latest.clone_from(&o3_data);

        let mut o4_data = None;
        while let Some((_, data)) = o4.next_if(|(index, _)| index <= &max_index) {
            o4_data = Some(data);
        }
        let o4_data = o4_data.or_else(|| o4_data_latest.take());
        o4_data_latest.clone_from(&o4_data);

        let mut o5_data = None;
        while let Some((_, data)) = o5.next_if(|(index, _)| index <= &max_index) {
            o5_data = Some(data);
        }
        let o5_data = o5_data.or_else(|| o5_data_latest.take());
        o5_data_latest.clone_from(&o5_data);

        let mut o6_data = None;
        while let Some((_, data)) = o6.next_if(|(index, _)| index <= &max_index) {
            o6_data = Some(data);
        }
        let o6_data = o6_data.or_else(|| o6_data_latest.take());
        o6_data_latest.clone_from(&o6_data);

        Some((
            max_index, r0_data, o0_data, o1_data, o2_data, o3_data, o4_data, o5_data, o6_data,
        ))
    }
}

/// Returns a new [`RangeZip1x8`] iterator.
///
/// The number of elements in a range zip iterator corresponds to the number of elements in the
/// shortest of its required iterators (`r0`).
///
/// Each call to `next` is guaranteed to yield the next value for each required iterator,
/// as well as the most recent index amongst all of them.
///
/// Optional iterators accumulate their state and yield their most recent value (if any),
/// each time the required iterators fire.
pub fn range_zip_1x8<
    Idx,
    IR0,
    R0,
    IO0,
    O0,
    IO1,
    O1,
    IO2,
    O2,
    IO3,
    O3,
    IO4,
    O4,
    IO5,
    O5,
    IO6,
    O6,
    IO7,
    O7,
>(
    r0: IR0,
    o0: IO0,
    o1: IO1,
    o2: IO2,
    o3: IO3,
    o4: IO4,
    o5: IO5,
    o6: IO6,
    o7: IO7,
) -> RangeZip1x8<
    Idx,
    IR0::IntoIter,
    R0,
    IO0::IntoIter,
    O0,
    IO1::IntoIter,
    O1,
    IO2::IntoIter,
    O2,
    IO3::IntoIter,
    O3,
    IO4::IntoIter,
    O4,
    IO5::IntoIter,
    O5,
    IO6::IntoIter,
    O6,
    IO7::IntoIter,
    O7,
>
where
    Idx: std::cmp::Ord,
    IR0: IntoIterator<Item = (Idx, R0)>,
    IO0: IntoIterator<Item = (Idx, O0)>,
    IO1: IntoIterator<Item = (Idx, O1)>,
    IO2: IntoIterator<Item = (Idx, O2)>,
    IO3: IntoIterator<Item = (Idx, O3)>,
    IO4: IntoIterator<Item = (Idx, O4)>,
    IO5: IntoIterator<Item = (Idx, O5)>,
    IO6: IntoIterator<Item = (Idx, O6)>,
    IO7: IntoIterator<Item = (Idx, O7)>,
{
    RangeZip1x8 {
        r0: r0.into_iter(),
        o0: o0.into_iter().peekable(),
        o1: o1.into_iter().peekable(),
        o2: o2.into_iter().peekable(),
        o3: o3.into_iter().peekable(),
        o4: o4.into_iter().peekable(),
        o5: o5.into_iter().peekable(),
        o6: o6.into_iter().peekable(),
        o7: o7.into_iter().peekable(),

        o0_data_latest: None,
        o1_data_latest: None,
        o2_data_latest: None,
        o3_data_latest: None,
        o4_data_latest: None,
        o5_data_latest: None,
        o6_data_latest: None,
        o7_data_latest: None,
    }
}

/// Implements a range zip iterator combinator with 2 required iterators and 2 optional
/// iterators.
///
/// See [`range_zip_1x8`] for more information.
pub struct RangeZip1x8<
    Idx,
    IR0,
    R0,
    IO0,
    O0,
    IO1,
    O1,
    IO2,
    O2,
    IO3,
    O3,
    IO4,
    O4,
    IO5,
    O5,
    IO6,
    O6,
    IO7,
    O7,
> where
    Idx: std::cmp::Ord,
    IR0: Iterator<Item = (Idx, R0)>,
    IO0: Iterator<Item = (Idx, O0)>,
    IO1: Iterator<Item = (Idx, O1)>,
    IO2: Iterator<Item = (Idx, O2)>,
    IO3: Iterator<Item = (Idx, O3)>,
    IO4: Iterator<Item = (Idx, O4)>,
    IO5: Iterator<Item = (Idx, O5)>,
    IO6: Iterator<Item = (Idx, O6)>,
    IO7: Iterator<Item = (Idx, O7)>,
{
    r0: IR0,
    o0: Peekable<IO0>,
    o1: Peekable<IO1>,
    o2: Peekable<IO2>,
    o3: Peekable<IO3>,
    o4: Peekable<IO4>,
    o5: Peekable<IO5>,
    o6: Peekable<IO6>,
    o7: Peekable<IO7>,

    o0_data_latest: Option<O0>,
    o1_data_latest: Option<O1>,
    o2_data_latest: Option<O2>,
    o3_data_latest: Option<O3>,
    o4_data_latest: Option<O4>,
    o5_data_latest: Option<O5>,
    o6_data_latest: Option<O6>,
    o7_data_latest: Option<O7>,
}

impl<Idx, IR0, R0, IO0, O0, IO1, O1, IO2, O2, IO3, O3, IO4, O4, IO5, O5, IO6, O6, IO7, O7> Iterator
    for RangeZip1x8<
        Idx,
        IR0,
        R0,
        IO0,
        O0,
        IO1,
        O1,
        IO2,
        O2,
        IO3,
        O3,
        IO4,
        O4,
        IO5,
        O5,
        IO6,
        O6,
        IO7,
        O7,
    >
where
    Idx: std::cmp::Ord,
    IR0: Iterator<Item = (Idx, R0)>,
    IO0: Iterator<Item = (Idx, O0)>,
    IO1: Iterator<Item = (Idx, O1)>,
    IO2: Iterator<Item = (Idx, O2)>,
    IO3: Iterator<Item = (Idx, O3)>,
    IO4: Iterator<Item = (Idx, O4)>,
    IO5: Iterator<Item = (Idx, O5)>,
    IO6: Iterator<Item = (Idx, O6)>,
    IO7: Iterator<Item = (Idx, O7)>,
    O0: Clone,
    O1: Clone,
    O2: Clone,
    O3: Clone,
    O4: Clone,
    O5: Clone,
    O6: Clone,
    O7: Clone,
{
    type Item = (
        Idx,
        R0,
        Option<O0>,
        Option<O1>,
        Option<O2>,
        Option<O3>,
        Option<O4>,
        Option<O5>,
        Option<O6>,
        Option<O7>,
    );

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let Self {
            r0,
            o0,
            o1,
            o2,
            o3,
            o4,
            o5,
            o6,
            o7,
            o0_data_latest,
            o1_data_latest,
            o2_data_latest,
            o3_data_latest,
            o4_data_latest,
            o5_data_latest,
            o6_data_latest,
            o7_data_latest,
        } = self;

        let (r0_index, r0_data) = r0.next()?;

        let max_index = [r0_index].into_iter().max()?;

        let mut o0_data = None;
        while let Some((_, data)) = o0.next_if(|(index, _)| index <= &max_index) {
            o0_data = Some(data);
        }
        let o0_data = o0_data.or_else(|| o0_data_latest.take());
        o0_data_latest.clone_from(&o0_data);

        let mut o1_data = None;
        while let Some((_, data)) = o1.next_if(|(index, _)| index <= &max_index) {
            o1_data = Some(data);
        }
        let o1_data = o1_data.or_else(|| o1_data_latest.take());
        o1_data_latest.clone_from(&o1_data);

        let mut o2_data = None;
        while let Some((_, data)) = o2.next_if(|(index, _)| index <= &max_index) {
            o2_data = Some(data);
        }
        let o2_data = o2_data.or_else(|| o2_data_latest.take());
        o2_data_latest.clone_from(&o2_data);

        let mut o3_data = None;
        while let Some((_, data)) = o3.next_if(|(index, _)| index <= &max_index) {
            o3_data = Some(data);
        }
        let o3_data = o3_data.or_else(|| o3_data_latest.take());
        o3_data_latest.clone_from(&o3_data);

        let mut o4_data = None;
        while let Some((_, data)) = o4.next_if(|(index, _)| index <= &max_index) {
            o4_data = Some(data);
        }
        let o4_data = o4_data.or_else(|| o4_data_latest.take());
        o4_data_latest.clone_from(&o4_data);

        let mut o5_data = None;
        while let Some((_, data)) = o5.next_if(|(index, _)| index <= &max_index) {
            o5_data = Some(data);
        }
        let o5_data = o5_data.or_else(|| o5_data_latest.take());
        o5_data_latest.clone_from(&o5_data);

        let mut o6_data = None;
        while let Some((_, data)) = o6.next_if(|(index, _)| index <= &max_index) {
            o6_data = Some(data);
        }
        let o6_data = o6_data.or_else(|| o6_data_latest.take());
        o6_data_latest.clone_from(&o6_data);

        let mut o7_data = None;
        while let Some((_, data)) = o7.next_if(|(index, _)| index <= &max_index) {
            o7_data = Some(data);
        }
        let o7_data = o7_data.or_else(|| o7_data_latest.take());
        o7_data_latest.clone_from(&o7_data);

        Some((
            max_index, r0_data, o0_data, o1_data, o2_data, o3_data, o4_data, o5_data, o6_data,
            o7_data,
        ))
    }
}

/// Returns a new [`RangeZip1x9`] iterator.
///
/// The number of elements in a range zip iterator corresponds to the number of elements in the
/// shortest of its required iterators (`r0`).
///
/// Each call to `next` is guaranteed to yield the next value for each required iterator,
/// as well as the most recent index amongst all of them.
///
/// Optional iterators accumulate their state and yield their most recent value (if any),
/// each time the required iterators fire.
pub fn range_zip_1x9<
    Idx,
    IR0,
    R0,
    IO0,
    O0,
    IO1,
    O1,
    IO2,
    O2,
    IO3,
    O3,
    IO4,
    O4,
    IO5,
    O5,
    IO6,
    O6,
    IO7,
    O7,
    IO8,
    O8,
>(
    r0: IR0,
    o0: IO0,
    o1: IO1,
    o2: IO2,
    o3: IO3,
    o4: IO4,
    o5: IO5,
    o6: IO6,
    o7: IO7,
    o8: IO8,
) -> RangeZip1x9<
    Idx,
    IR0::IntoIter,
    R0,
    IO0::IntoIter,
    O0,
    IO1::IntoIter,
    O1,
    IO2::IntoIter,
    O2,
    IO3::IntoIter,
    O3,
    IO4::IntoIter,
    O4,
    IO5::IntoIter,
    O5,
    IO6::IntoIter,
    O6,
    IO7::IntoIter,
    O7,
    IO8::IntoIter,
    O8,
>
where
    Idx: std::cmp::Ord,
    IR0: IntoIterator<Item = (Idx, R0)>,
    IO0: IntoIterator<Item = (Idx, O0)>,
    IO1: IntoIterator<Item = (Idx, O1)>,
    IO2: IntoIterator<Item = (Idx, O2)>,
    IO3: IntoIterator<Item = (Idx, O3)>,
    IO4: IntoIterator<Item = (Idx, O4)>,
    IO5: IntoIterator<Item = (Idx, O5)>,
    IO6: IntoIterator<Item = (Idx, O6)>,
    IO7: IntoIterator<Item = (Idx, O7)>,
    IO8: IntoIterator<Item = (Idx, O8)>,
{
    RangeZip1x9 {
        r0: r0.into_iter(),
        o0: o0.into_iter().peekable(),
        o1: o1.into_iter().peekable(),
        o2: o2.into_iter().peekable(),
        o3: o3.into_iter().peekable(),
        o4: o4.into_iter().peekable(),
        o5: o5.into_iter().peekable(),
        o6: o6.into_iter().peekable(),
        o7: o7.into_iter().peekable(),
        o8: o8.into_iter().peekable(),

        o0_data_latest: None,
        o1_data_latest: None,
        o2_data_latest: None,
        o3_data_latest: None,
        o4_data_latest: None,
        o5_data_latest: None,
        o6_data_latest: None,
        o7_data_latest: None,
        o8_data_latest: None,
    }
}

/// Implements a range zip iterator combinator with 2 required iterators and 2 optional
/// iterators.
///
/// See [`range_zip_1x9`] for more information.
pub struct RangeZip1x9<
    Idx,
    IR0,
    R0,
    IO0,
    O0,
    IO1,
    O1,
    IO2,
    O2,
    IO3,
    O3,
    IO4,
    O4,
    IO5,
    O5,
    IO6,
    O6,
    IO7,
    O7,
    IO8,
    O8,
> where
    Idx: std::cmp::Ord,
    IR0: Iterator<Item = (Idx, R0)>,
    IO0: Iterator<Item = (Idx, O0)>,
    IO1: Iterator<Item = (Idx, O1)>,
    IO2: Iterator<Item = (Idx, O2)>,
    IO3: Iterator<Item = (Idx, O3)>,
    IO4: Iterator<Item = (Idx, O4)>,
    IO5: Iterator<Item = (Idx, O5)>,
    IO6: Iterator<Item = (Idx, O6)>,
    IO7: Iterator<Item = (Idx, O7)>,
    IO8: Iterator<Item = (Idx, O8)>,
{
    r0: IR0,
    o0: Peekable<IO0>,
    o1: Peekable<IO1>,
    o2: Peekable<IO2>,
    o3: Peekable<IO3>,
    o4: Peekable<IO4>,
    o5: Peekable<IO5>,
    o6: Peekable<IO6>,
    o7: Peekable<IO7>,
    o8: Peekable<IO8>,

    o0_data_latest: Option<O0>,
    o1_data_latest: Option<O1>,
    o2_data_latest: Option<O2>,
    o3_data_latest: Option<O3>,
    o4_data_latest: Option<O4>,
    o5_data_latest: Option<O5>,
    o6_data_latest: Option<O6>,
    o7_data_latest: Option<O7>,
    o8_data_latest: Option<O8>,
}

impl<Idx, IR0, R0, IO0, O0, IO1, O1, IO2, O2, IO3, O3, IO4, O4, IO5, O5, IO6, O6, IO7, O7, IO8, O8>
    Iterator
    for RangeZip1x9<
        Idx,
        IR0,
        R0,
        IO0,
        O0,
        IO1,
        O1,
        IO2,
        O2,
        IO3,
        O3,
        IO4,
        O4,
        IO5,
        O5,
        IO6,
        O6,
        IO7,
        O7,
        IO8,
        O8,
    >
where
    Idx: std::cmp::Ord,
    IR0: Iterator<Item = (Idx, R0)>,
    IO0: Iterator<Item = (Idx, O0)>,
    IO1: Iterator<Item = (Idx, O1)>,
    IO2: Iterator<Item = (Idx, O2)>,
    IO3: Iterator<Item = (Idx, O3)>,
    IO4: Iterator<Item = (Idx, O4)>,
    IO5: Iterator<Item = (Idx, O5)>,
    IO6: Iterator<Item = (Idx, O6)>,
    IO7: Iterator<Item = (Idx, O7)>,
    IO8: Iterator<Item = (Idx, O8)>,
    O0: Clone,
    O1: Clone,
    O2: Clone,
    O3: Clone,
    O4: Clone,
    O5: Clone,
    O6: Clone,
    O7: Clone,
    O8: Clone,
{
    type Item = (
        Idx,
        R0,
        Option<O0>,
        Option<O1>,
        Option<O2>,
        Option<O3>,
        Option<O4>,
        Option<O5>,
        Option<O6>,
        Option<O7>,
        Option<O8>,
    );

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let Self {
            r0,
            o0,
            o1,
            o2,
            o3,
            o4,
            o5,
            o6,
            o7,
            o8,
            o0_data_latest,
            o1_data_latest,
            o2_data_latest,
            o3_data_latest,
            o4_data_latest,
            o5_data_latest,
            o6_data_latest,
            o7_data_latest,
            o8_data_latest,
        } = self;

        let (r0_index, r0_data) = r0.next()?;

        let max_index = [r0_index].into_iter().max()?;

        let mut o0_data = None;
        while let Some((_, data)) = o0.next_if(|(index, _)| index <= &max_index) {
            o0_data = Some(data);
        }
        let o0_data = o0_data.or_else(|| o0_data_latest.take());
        o0_data_latest.clone_from(&o0_data);

        let mut o1_data = None;
        while let Some((_, data)) = o1.next_if(|(index, _)| index <= &max_index) {
            o1_data = Some(data);
        }
        let o1_data = o1_data.or_else(|| o1_data_latest.take());
        o1_data_latest.clone_from(&o1_data);

        let mut o2_data = None;
        while let Some((_, data)) = o2.next_if(|(index, _)| index <= &max_index) {
            o2_data = Some(data);
        }
        let o2_data = o2_data.or_else(|| o2_data_latest.take());
        o2_data_latest.clone_from(&o2_data);

        let mut o3_data = None;
        while let Some((_, data)) = o3.next_if(|(index, _)| index <= &max_index) {
            o3_data = Some(data);
        }
        let o3_data = o3_data.or_else(|| o3_data_latest.take());
        o3_data_latest.clone_from(&o3_data);

        let mut o4_data = None;
        while let Some((_, data)) = o4.next_if(|(index, _)| index <= &max_index) {
            o4_data = Some(data);
        }
        let o4_data = o4_data.or_else(|| o4_data_latest.take());
        o4_data_latest.clone_from(&o4_data);

        let mut o5_data = None;
        while let Some((_, data)) = o5.next_if(|(index, _)| index <= &max_index) {
            o5_data = Some(data);
        }
        let o5_data = o5_data.or_else(|| o5_data_latest.take());
        o5_data_latest.clone_from(&o5_data);

        let mut o6_data = None;
        while let Some((_, data)) = o6.next_if(|(index, _)| index <= &max_index) {
            o6_data = Some(data);
        }
        let o6_data = o6_data.or_else(|| o6_data_latest.take());
        o6_data_latest.clone_from(&o6_data);

        let mut o7_data = None;
        while let Some((_, data)) = o7.next_if(|(index, _)| index <= &max_index) {
            o7_data = Some(data);
        }
        let o7_data = o7_data.or_else(|| o7_data_latest.take());
        o7_data_latest.clone_from(&o7_data);

        let mut o8_data = None;
        while let Some((_, data)) = o8.next_if(|(index, _)| index <= &max_index) {
            o8_data = Some(data);
        }
        let o8_data = o8_data.or_else(|| o8_data_latest.take());
        o8_data_latest.clone_from(&o8_data);

        Some((
            max_index, r0_data, o0_data, o1_data, o2_data, o3_data, o4_data, o5_data, o6_data,
            o7_data, o8_data,
        ))
    }
}

/// Returns a new [`RangeZip2x1`] iterator.
///
/// The number of elements in a range zip iterator corresponds to the number of elements in the
/// shortest of its required iterators (`r0`, `r1`).
///
/// Each call to `next` is guaranteed to yield the next value for each required iterator,
/// as well as the most recent index amongst all of them.
///
/// Optional iterators accumulate their state and yield their most recent value (if any),
/// each time the required iterators fire.
pub fn range_zip_2x1<Idx, IR0, R0, IR1, R1, IO0, O0>(
    r0: IR0,
    r1: IR1,
    o0: IO0,
) -> RangeZip2x1<Idx, IR0::IntoIter, R0, IR1::IntoIter, R1, IO0::IntoIter, O0>
where
    Idx: std::cmp::Ord,
    IR0: IntoIterator<Item = (Idx, R0)>,
    IR1: IntoIterator<Item = (Idx, R1)>,
    IO0: IntoIterator<Item = (Idx, O0)>,
{
    RangeZip2x1 {
        r0: r0.into_iter(),
        r1: r1.into_iter(),
        o0: o0.into_iter().peekable(),

        o0_data_latest: None,
    }
}

/// Implements a range zip iterator combinator with 2 required iterators and 2 optional
/// iterators.
///
/// See [`range_zip_2x1`] for more information.
pub struct RangeZip2x1<Idx, IR0, R0, IR1, R1, IO0, O0>
where
    Idx: std::cmp::Ord,
    IR0: Iterator<Item = (Idx, R0)>,
    IR1: Iterator<Item = (Idx, R1)>,
    IO0: Iterator<Item = (Idx, O0)>,
{
    r0: IR0,
    r1: IR1,
    o0: Peekable<IO0>,

    o0_data_latest: Option<O0>,
}

impl<Idx, IR0, R0, IR1, R1, IO0, O0> Iterator for RangeZip2x1<Idx, IR0, R0, IR1, R1, IO0, O0>
where
    Idx: std::cmp::Ord,
    IR0: Iterator<Item = (Idx, R0)>,
    IR1: Iterator<Item = (Idx, R1)>,
    IO0: Iterator<Item = (Idx, O0)>,
    O0: Clone,
{
    type Item = (Idx, R0, R1, Option<O0>);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let Self {
            r0,
            r1,
            o0,
            o0_data_latest,
        } = self;

        let (r0_index, r0_data) = r0.next()?;
        let (r1_index, r1_data) = r1.next()?;

        let max_index = [r0_index, r1_index].into_iter().max()?;

        let mut o0_data = None;
        while let Some((_, data)) = o0.next_if(|(index, _)| index <= &max_index) {
            o0_data = Some(data);
        }
        let o0_data = o0_data.or_else(|| o0_data_latest.take());
        o0_data_latest.clone_from(&o0_data);

        Some((max_index, r0_data, r1_data, o0_data))
    }
}

/// Returns a new [`RangeZip2x2`] iterator.
///
/// The number of elements in a range zip iterator corresponds to the number of elements in the
/// shortest of its required iterators (`r0`, `r1`).
///
/// Each call to `next` is guaranteed to yield the next value for each required iterator,
/// as well as the most recent index amongst all of them.
///
/// Optional iterators accumulate their state and yield their most recent value (if any),
/// each time the required iterators fire.
pub fn range_zip_2x2<Idx, IR0, R0, IR1, R1, IO0, O0, IO1, O1>(
    r0: IR0,
    r1: IR1,
    o0: IO0,
    o1: IO1,
) -> RangeZip2x2<Idx, IR0::IntoIter, R0, IR1::IntoIter, R1, IO0::IntoIter, O0, IO1::IntoIter, O1>
where
    Idx: std::cmp::Ord,
    IR0: IntoIterator<Item = (Idx, R0)>,
    IR1: IntoIterator<Item = (Idx, R1)>,
    IO0: IntoIterator<Item = (Idx, O0)>,
    IO1: IntoIterator<Item = (Idx, O1)>,
{
    RangeZip2x2 {
        r0: r0.into_iter(),
        r1: r1.into_iter(),
        o0: o0.into_iter().peekable(),
        o1: o1.into_iter().peekable(),

        o0_data_latest: None,
        o1_data_latest: None,
    }
}

/// Implements a range zip iterator combinator with 2 required iterators and 2 optional
/// iterators.
///
/// See [`range_zip_2x2`] for more information.
pub struct RangeZip2x2<Idx, IR0, R0, IR1, R1, IO0, O0, IO1, O1>
where
    Idx: std::cmp::Ord,
    IR0: Iterator<Item = (Idx, R0)>,
    IR1: Iterator<Item = (Idx, R1)>,
    IO0: Iterator<Item = (Idx, O0)>,
    IO1: Iterator<Item = (Idx, O1)>,
{
    r0: IR0,
    r1: IR1,
    o0: Peekable<IO0>,
    o1: Peekable<IO1>,

    o0_data_latest: Option<O0>,
    o1_data_latest: Option<O1>,
}

impl<Idx, IR0, R0, IR1, R1, IO0, O0, IO1, O1> Iterator
    for RangeZip2x2<Idx, IR0, R0, IR1, R1, IO0, O0, IO1, O1>
where
    Idx: std::cmp::Ord,
    IR0: Iterator<Item = (Idx, R0)>,
    IR1: Iterator<Item = (Idx, R1)>,
    IO0: Iterator<Item = (Idx, O0)>,
    IO1: Iterator<Item = (Idx, O1)>,
    O0: Clone,
    O1: Clone,
{
    type Item = (Idx, R0, R1, Option<O0>, Option<O1>);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let Self {
            r0,
            r1,
            o0,
            o1,
            o0_data_latest,
            o1_data_latest,
        } = self;

        let (r0_index, r0_data) = r0.next()?;
        let (r1_index, r1_data) = r1.next()?;

        let max_index = [r0_index, r1_index].into_iter().max()?;

        let mut o0_data = None;
        while let Some((_, data)) = o0.next_if(|(index, _)| index <= &max_index) {
            o0_data = Some(data);
        }
        let o0_data = o0_data.or_else(|| o0_data_latest.take());
        o0_data_latest.clone_from(&o0_data);

        let mut o1_data = None;
        while let Some((_, data)) = o1.next_if(|(index, _)| index <= &max_index) {
            o1_data = Some(data);
        }
        let o1_data = o1_data.or_else(|| o1_data_latest.take());
        o1_data_latest.clone_from(&o1_data);

        Some((max_index, r0_data, r1_data, o0_data, o1_data))
    }
}

/// Returns a new [`RangeZip2x3`] iterator.
///
/// The number of elements in a range zip iterator corresponds to the number of elements in the
/// shortest of its required iterators (`r0`, `r1`).
///
/// Each call to `next` is guaranteed to yield the next value for each required iterator,
/// as well as the most recent index amongst all of them.
///
/// Optional iterators accumulate their state and yield their most recent value (if any),
/// each time the required iterators fire.
pub fn range_zip_2x3<Idx, IR0, R0, IR1, R1, IO0, O0, IO1, O1, IO2, O2>(
    r0: IR0,
    r1: IR1,
    o0: IO0,
    o1: IO1,
    o2: IO2,
) -> RangeZip2x3<
    Idx,
    IR0::IntoIter,
    R0,
    IR1::IntoIter,
    R1,
    IO0::IntoIter,
    O0,
    IO1::IntoIter,
    O1,
    IO2::IntoIter,
    O2,
>
where
    Idx: std::cmp::Ord,
    IR0: IntoIterator<Item = (Idx, R0)>,
    IR1: IntoIterator<Item = (Idx, R1)>,
    IO0: IntoIterator<Item = (Idx, O0)>,
    IO1: IntoIterator<Item = (Idx, O1)>,
    IO2: IntoIterator<Item = (Idx, O2)>,
{
    RangeZip2x3 {
        r0: r0.into_iter(),
        r1: r1.into_iter(),
        o0: o0.into_iter().peekable(),
        o1: o1.into_iter().peekable(),
        o2: o2.into_iter().peekable(),

        o0_data_latest: None,
        o1_data_latest: None,
        o2_data_latest: None,
    }
}

/// Implements a range zip iterator combinator with 2 required iterators and 2 optional
/// iterators.
///
/// See [`range_zip_2x3`] for more information.
pub struct RangeZip2x3<Idx, IR0, R0, IR1, R1, IO0, O0, IO1, O1, IO2, O2>
where
    Idx: std::cmp::Ord,
    IR0: Iterator<Item = (Idx, R0)>,
    IR1: Iterator<Item = (Idx, R1)>,
    IO0: Iterator<Item = (Idx, O0)>,
    IO1: Iterator<Item = (Idx, O1)>,
    IO2: Iterator<Item = (Idx, O2)>,
{
    r0: IR0,
    r1: IR1,
    o0: Peekable<IO0>,
    o1: Peekable<IO1>,
    o2: Peekable<IO2>,

    o0_data_latest: Option<O0>,
    o1_data_latest: Option<O1>,
    o2_data_latest: Option<O2>,
}

impl<Idx, IR0, R0, IR1, R1, IO0, O0, IO1, O1, IO2, O2> Iterator
    for RangeZip2x3<Idx, IR0, R0, IR1, R1, IO0, O0, IO1, O1, IO2, O2>
where
    Idx: std::cmp::Ord,
    IR0: Iterator<Item = (Idx, R0)>,
    IR1: Iterator<Item = (Idx, R1)>,
    IO0: Iterator<Item = (Idx, O0)>,
    IO1: Iterator<Item = (Idx, O1)>,
    IO2: Iterator<Item = (Idx, O2)>,
    O0: Clone,
    O1: Clone,
    O2: Clone,
{
    type Item = (Idx, R0, R1, Option<O0>, Option<O1>, Option<O2>);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let Self {
            r0,
            r1,
            o0,
            o1,
            o2,
            o0_data_latest,
            o1_data_latest,
            o2_data_latest,
        } = self;

        let (r0_index, r0_data) = r0.next()?;
        let (r1_index, r1_data) = r1.next()?;

        let max_index = [r0_index, r1_index].into_iter().max()?;

        let mut o0_data = None;
        while let Some((_, data)) = o0.next_if(|(index, _)| index <= &max_index) {
            o0_data = Some(data);
        }
        let o0_data = o0_data.or_else(|| o0_data_latest.take());
        o0_data_latest.clone_from(&o0_data);

        let mut o1_data = None;
        while let Some((_, data)) = o1.next_if(|(index, _)| index <= &max_index) {
            o1_data = Some(data);
        }
        let o1_data = o1_data.or_else(|| o1_data_latest.take());
        o1_data_latest.clone_from(&o1_data);

        let mut o2_data = None;
        while let Some((_, data)) = o2.next_if(|(index, _)| index <= &max_index) {
            o2_data = Some(data);
        }
        let o2_data = o2_data.or_else(|| o2_data_latest.take());
        o2_data_latest.clone_from(&o2_data);

        Some((max_index, r0_data, r1_data, o0_data, o1_data, o2_data))
    }
}

/// Returns a new [`RangeZip2x4`] iterator.
///
/// The number of elements in a range zip iterator corresponds to the number of elements in the
/// shortest of its required iterators (`r0`, `r1`).
///
/// Each call to `next` is guaranteed to yield the next value for each required iterator,
/// as well as the most recent index amongst all of them.
///
/// Optional iterators accumulate their state and yield their most recent value (if any),
/// each time the required iterators fire.
pub fn range_zip_2x4<Idx, IR0, R0, IR1, R1, IO0, O0, IO1, O1, IO2, O2, IO3, O3>(
    r0: IR0,
    r1: IR1,
    o0: IO0,
    o1: IO1,
    o2: IO2,
    o3: IO3,
) -> RangeZip2x4<
    Idx,
    IR0::IntoIter,
    R0,
    IR1::IntoIter,
    R1,
    IO0::IntoIter,
    O0,
    IO1::IntoIter,
    O1,
    IO2::IntoIter,
    O2,
    IO3::IntoIter,
    O3,
>
where
    Idx: std::cmp::Ord,
    IR0: IntoIterator<Item = (Idx, R0)>,
    IR1: IntoIterator<Item = (Idx, R1)>,
    IO0: IntoIterator<Item = (Idx, O0)>,
    IO1: IntoIterator<Item = (Idx, O1)>,
    IO2: IntoIterator<Item = (Idx, O2)>,
    IO3: IntoIterator<Item = (Idx, O3)>,
{
    RangeZip2x4 {
        r0: r0.into_iter(),
        r1: r1.into_iter(),
        o0: o0.into_iter().peekable(),
        o1: o1.into_iter().peekable(),
        o2: o2.into_iter().peekable(),
        o3: o3.into_iter().peekable(),

        o0_data_latest: None,
        o1_data_latest: None,
        o2_data_latest: None,
        o3_data_latest: None,
    }
}

/// Implements a range zip iterator combinator with 2 required iterators and 2 optional
/// iterators.
///
/// See [`range_zip_2x4`] for more information.
pub struct RangeZip2x4<Idx, IR0, R0, IR1, R1, IO0, O0, IO1, O1, IO2, O2, IO3, O3>
where
    Idx: std::cmp::Ord,
    IR0: Iterator<Item = (Idx, R0)>,
    IR1: Iterator<Item = (Idx, R1)>,
    IO0: Iterator<Item = (Idx, O0)>,
    IO1: Iterator<Item = (Idx, O1)>,
    IO2: Iterator<Item = (Idx, O2)>,
    IO3: Iterator<Item = (Idx, O3)>,
{
    r0: IR0,
    r1: IR1,
    o0: Peekable<IO0>,
    o1: Peekable<IO1>,
    o2: Peekable<IO2>,
    o3: Peekable<IO3>,

    o0_data_latest: Option<O0>,
    o1_data_latest: Option<O1>,
    o2_data_latest: Option<O2>,
    o3_data_latest: Option<O3>,
}

impl<Idx, IR0, R0, IR1, R1, IO0, O0, IO1, O1, IO2, O2, IO3, O3> Iterator
    for RangeZip2x4<Idx, IR0, R0, IR1, R1, IO0, O0, IO1, O1, IO2, O2, IO3, O3>
where
    Idx: std::cmp::Ord,
    IR0: Iterator<Item = (Idx, R0)>,
    IR1: Iterator<Item = (Idx, R1)>,
    IO0: Iterator<Item = (Idx, O0)>,
    IO1: Iterator<Item = (Idx, O1)>,
    IO2: Iterator<Item = (Idx, O2)>,
    IO3: Iterator<Item = (Idx, O3)>,
    O0: Clone,
    O1: Clone,
    O2: Clone,
    O3: Clone,
{
    type Item = (Idx, R0, R1, Option<O0>, Option<O1>, Option<O2>, Option<O3>);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let Self {
            r0,
            r1,
            o0,
            o1,
            o2,
            o3,
            o0_data_latest,
            o1_data_latest,
            o2_data_latest,
            o3_data_latest,
        } = self;

        let (r0_index, r0_data) = r0.next()?;
        let (r1_index, r1_data) = r1.next()?;

        let max_index = [r0_index, r1_index].into_iter().max()?;

        let mut o0_data = None;
        while let Some((_, data)) = o0.next_if(|(index, _)| index <= &max_index) {
            o0_data = Some(data);
        }
        let o0_data = o0_data.or_else(|| o0_data_latest.take());
        o0_data_latest.clone_from(&o0_data);

        let mut o1_data = None;
        while let Some((_, data)) = o1.next_if(|(index, _)| index <= &max_index) {
            o1_data = Some(data);
        }
        let o1_data = o1_data.or_else(|| o1_data_latest.take());
        o1_data_latest.clone_from(&o1_data);

        let mut o2_data = None;
        while let Some((_, data)) = o2.next_if(|(index, _)| index <= &max_index) {
            o2_data = Some(data);
        }
        let o2_data = o2_data.or_else(|| o2_data_latest.take());
        o2_data_latest.clone_from(&o2_data);

        let mut o3_data = None;
        while let Some((_, data)) = o3.next_if(|(index, _)| index <= &max_index) {
            o3_data = Some(data);
        }
        let o3_data = o3_data.or_else(|| o3_data_latest.take());
        o3_data_latest.clone_from(&o3_data);

        Some((
            max_index, r0_data, r1_data, o0_data, o1_data, o2_data, o3_data,
        ))
    }
}

/// Returns a new [`RangeZip2x5`] iterator.
///
/// The number of elements in a range zip iterator corresponds to the number of elements in the
/// shortest of its required iterators (`r0`, `r1`).
///
/// Each call to `next` is guaranteed to yield the next value for each required iterator,
/// as well as the most recent index amongst all of them.
///
/// Optional iterators accumulate their state and yield their most recent value (if any),
/// each time the required iterators fire.
pub fn range_zip_2x5<Idx, IR0, R0, IR1, R1, IO0, O0, IO1, O1, IO2, O2, IO3, O3, IO4, O4>(
    r0: IR0,
    r1: IR1,
    o0: IO0,
    o1: IO1,
    o2: IO2,
    o3: IO3,
    o4: IO4,
) -> RangeZip2x5<
    Idx,
    IR0::IntoIter,
    R0,
    IR1::IntoIter,
    R1,
    IO0::IntoIter,
    O0,
    IO1::IntoIter,
    O1,
    IO2::IntoIter,
    O2,
    IO3::IntoIter,
    O3,
    IO4::IntoIter,
    O4,
>
where
    Idx: std::cmp::Ord,
    IR0: IntoIterator<Item = (Idx, R0)>,
    IR1: IntoIterator<Item = (Idx, R1)>,
    IO0: IntoIterator<Item = (Idx, O0)>,
    IO1: IntoIterator<Item = (Idx, O1)>,
    IO2: IntoIterator<Item = (Idx, O2)>,
    IO3: IntoIterator<Item = (Idx, O3)>,
    IO4: IntoIterator<Item = (Idx, O4)>,
{
    RangeZip2x5 {
        r0: r0.into_iter(),
        r1: r1.into_iter(),
        o0: o0.into_iter().peekable(),
        o1: o1.into_iter().peekable(),
        o2: o2.into_iter().peekable(),
        o3: o3.into_iter().peekable(),
        o4: o4.into_iter().peekable(),

        o0_data_latest: None,
        o1_data_latest: None,
        o2_data_latest: None,
        o3_data_latest: None,
        o4_data_latest: None,
    }
}

/// Implements a range zip iterator combinator with 2 required iterators and 2 optional
/// iterators.
///
/// See [`range_zip_2x5`] for more information.
pub struct RangeZip2x5<Idx, IR0, R0, IR1, R1, IO0, O0, IO1, O1, IO2, O2, IO3, O3, IO4, O4>
where
    Idx: std::cmp::Ord,
    IR0: Iterator<Item = (Idx, R0)>,
    IR1: Iterator<Item = (Idx, R1)>,
    IO0: Iterator<Item = (Idx, O0)>,
    IO1: Iterator<Item = (Idx, O1)>,
    IO2: Iterator<Item = (Idx, O2)>,
    IO3: Iterator<Item = (Idx, O3)>,
    IO4: Iterator<Item = (Idx, O4)>,
{
    r0: IR0,
    r1: IR1,
    o0: Peekable<IO0>,
    o1: Peekable<IO1>,
    o2: Peekable<IO2>,
    o3: Peekable<IO3>,
    o4: Peekable<IO4>,

    o0_data_latest: Option<O0>,
    o1_data_latest: Option<O1>,
    o2_data_latest: Option<O2>,
    o3_data_latest: Option<O3>,
    o4_data_latest: Option<O4>,
}

impl<Idx, IR0, R0, IR1, R1, IO0, O0, IO1, O1, IO2, O2, IO3, O3, IO4, O4> Iterator
    for RangeZip2x5<Idx, IR0, R0, IR1, R1, IO0, O0, IO1, O1, IO2, O2, IO3, O3, IO4, O4>
where
    Idx: std::cmp::Ord,
    IR0: Iterator<Item = (Idx, R0)>,
    IR1: Iterator<Item = (Idx, R1)>,
    IO0: Iterator<Item = (Idx, O0)>,
    IO1: Iterator<Item = (Idx, O1)>,
    IO2: Iterator<Item = (Idx, O2)>,
    IO3: Iterator<Item = (Idx, O3)>,
    IO4: Iterator<Item = (Idx, O4)>,
    O0: Clone,
    O1: Clone,
    O2: Clone,
    O3: Clone,
    O4: Clone,
{
    type Item = (
        Idx,
        R0,
        R1,
        Option<O0>,
        Option<O1>,
        Option<O2>,
        Option<O3>,
        Option<O4>,
    );

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let Self {
            r0,
            r1,
            o0,
            o1,
            o2,
            o3,
            o4,
            o0_data_latest,
            o1_data_latest,
            o2_data_latest,
            o3_data_latest,
            o4_data_latest,
        } = self;

        let (r0_index, r0_data) = r0.next()?;
        let (r1_index, r1_data) = r1.next()?;

        let max_index = [r0_index, r1_index].into_iter().max()?;

        let mut o0_data = None;
        while let Some((_, data)) = o0.next_if(|(index, _)| index <= &max_index) {
            o0_data = Some(data);
        }
        let o0_data = o0_data.or_else(|| o0_data_latest.take());
        o0_data_latest.clone_from(&o0_data);

        let mut o1_data = None;
        while let Some((_, data)) = o1.next_if(|(index, _)| index <= &max_index) {
            o1_data = Some(data);
        }
        let o1_data = o1_data.or_else(|| o1_data_latest.take());
        o1_data_latest.clone_from(&o1_data);

        let mut o2_data = None;
        while let Some((_, data)) = o2.next_if(|(index, _)| index <= &max_index) {
            o2_data = Some(data);
        }
        let o2_data = o2_data.or_else(|| o2_data_latest.take());
        o2_data_latest.clone_from(&o2_data);

        let mut o3_data = None;
        while let Some((_, data)) = o3.next_if(|(index, _)| index <= &max_index) {
            o3_data = Some(data);
        }
        let o3_data = o3_data.or_else(|| o3_data_latest.take());
        o3_data_latest.clone_from(&o3_data);

        let mut o4_data = None;
        while let Some((_, data)) = o4.next_if(|(index, _)| index <= &max_index) {
            o4_data = Some(data);
        }
        let o4_data = o4_data.or_else(|| o4_data_latest.take());
        o4_data_latest.clone_from(&o4_data);

        Some((
            max_index, r0_data, r1_data, o0_data, o1_data, o2_data, o3_data, o4_data,
        ))
    }
}

/// Returns a new [`RangeZip2x6`] iterator.
///
/// The number of elements in a range zip iterator corresponds to the number of elements in the
/// shortest of its required iterators (`r0`, `r1`).
///
/// Each call to `next` is guaranteed to yield the next value for each required iterator,
/// as well as the most recent index amongst all of them.
///
/// Optional iterators accumulate their state and yield their most recent value (if any),
/// each time the required iterators fire.
pub fn range_zip_2x6<Idx, IR0, R0, IR1, R1, IO0, O0, IO1, O1, IO2, O2, IO3, O3, IO4, O4, IO5, O5>(
    r0: IR0,
    r1: IR1,
    o0: IO0,
    o1: IO1,
    o2: IO2,
    o3: IO3,
    o4: IO4,
    o5: IO5,
) -> RangeZip2x6<
    Idx,
    IR0::IntoIter,
    R0,
    IR1::IntoIter,
    R1,
    IO0::IntoIter,
    O0,
    IO1::IntoIter,
    O1,
    IO2::IntoIter,
    O2,
    IO3::IntoIter,
    O3,
    IO4::IntoIter,
    O4,
    IO5::IntoIter,
    O5,
>
where
    Idx: std::cmp::Ord,
    IR0: IntoIterator<Item = (Idx, R0)>,
    IR1: IntoIterator<Item = (Idx, R1)>,
    IO0: IntoIterator<Item = (Idx, O0)>,
    IO1: IntoIterator<Item = (Idx, O1)>,
    IO2: IntoIterator<Item = (Idx, O2)>,
    IO3: IntoIterator<Item = (Idx, O3)>,
    IO4: IntoIterator<Item = (Idx, O4)>,
    IO5: IntoIterator<Item = (Idx, O5)>,
{
    RangeZip2x6 {
        r0: r0.into_iter(),
        r1: r1.into_iter(),
        o0: o0.into_iter().peekable(),
        o1: o1.into_iter().peekable(),
        o2: o2.into_iter().peekable(),
        o3: o3.into_iter().peekable(),
        o4: o4.into_iter().peekable(),
        o5: o5.into_iter().peekable(),

        o0_data_latest: None,
        o1_data_latest: None,
        o2_data_latest: None,
        o3_data_latest: None,
        o4_data_latest: None,
        o5_data_latest: None,
    }
}

/// Implements a range zip iterator combinator with 2 required iterators and 2 optional
/// iterators.
///
/// See [`range_zip_2x6`] for more information.
pub struct RangeZip2x6<Idx, IR0, R0, IR1, R1, IO0, O0, IO1, O1, IO2, O2, IO3, O3, IO4, O4, IO5, O5>
where
    Idx: std::cmp::Ord,
    IR0: Iterator<Item = (Idx, R0)>,
    IR1: Iterator<Item = (Idx, R1)>,
    IO0: Iterator<Item = (Idx, O0)>,
    IO1: Iterator<Item = (Idx, O1)>,
    IO2: Iterator<Item = (Idx, O2)>,
    IO3: Iterator<Item = (Idx, O3)>,
    IO4: Iterator<Item = (Idx, O4)>,
    IO5: Iterator<Item = (Idx, O5)>,
{
    r0: IR0,
    r1: IR1,
    o0: Peekable<IO0>,
    o1: Peekable<IO1>,
    o2: Peekable<IO2>,
    o3: Peekable<IO3>,
    o4: Peekable<IO4>,
    o5: Peekable<IO5>,

    o0_data_latest: Option<O0>,
    o1_data_latest: Option<O1>,
    o2_data_latest: Option<O2>,
    o3_data_latest: Option<O3>,
    o4_data_latest: Option<O4>,
    o5_data_latest: Option<O5>,
}

impl<Idx, IR0, R0, IR1, R1, IO0, O0, IO1, O1, IO2, O2, IO3, O3, IO4, O4, IO5, O5> Iterator
    for RangeZip2x6<Idx, IR0, R0, IR1, R1, IO0, O0, IO1, O1, IO2, O2, IO3, O3, IO4, O4, IO5, O5>
where
    Idx: std::cmp::Ord,
    IR0: Iterator<Item = (Idx, R0)>,
    IR1: Iterator<Item = (Idx, R1)>,
    IO0: Iterator<Item = (Idx, O0)>,
    IO1: Iterator<Item = (Idx, O1)>,
    IO2: Iterator<Item = (Idx, O2)>,
    IO3: Iterator<Item = (Idx, O3)>,
    IO4: Iterator<Item = (Idx, O4)>,
    IO5: Iterator<Item = (Idx, O5)>,
    O0: Clone,
    O1: Clone,
    O2: Clone,
    O3: Clone,
    O4: Clone,
    O5: Clone,
{
    type Item = (
        Idx,
        R0,
        R1,
        Option<O0>,
        Option<O1>,
        Option<O2>,
        Option<O3>,
        Option<O4>,
        Option<O5>,
    );

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let Self {
            r0,
            r1,
            o0,
            o1,
            o2,
            o3,
            o4,
            o5,
            o0_data_latest,
            o1_data_latest,
            o2_data_latest,
            o3_data_latest,
            o4_data_latest,
            o5_data_latest,
        } = self;

        let (r0_index, r0_data) = r0.next()?;
        let (r1_index, r1_data) = r1.next()?;

        let max_index = [r0_index, r1_index].into_iter().max()?;

        let mut o0_data = None;
        while let Some((_, data)) = o0.next_if(|(index, _)| index <= &max_index) {
            o0_data = Some(data);
        }
        let o0_data = o0_data.or_else(|| o0_data_latest.take());
        o0_data_latest.clone_from(&o0_data);

        let mut o1_data = None;
        while let Some((_, data)) = o1.next_if(|(index, _)| index <= &max_index) {
            o1_data = Some(data);
        }
        let o1_data = o1_data.or_else(|| o1_data_latest.take());
        o1_data_latest.clone_from(&o1_data);

        let mut o2_data = None;
        while let Some((_, data)) = o2.next_if(|(index, _)| index <= &max_index) {
            o2_data = Some(data);
        }
        let o2_data = o2_data.or_else(|| o2_data_latest.take());
        o2_data_latest.clone_from(&o2_data);

        let mut o3_data = None;
        while let Some((_, data)) = o3.next_if(|(index, _)| index <= &max_index) {
            o3_data = Some(data);
        }
        let o3_data = o3_data.or_else(|| o3_data_latest.take());
        o3_data_latest.clone_from(&o3_data);

        let mut o4_data = None;
        while let Some((_, data)) = o4.next_if(|(index, _)| index <= &max_index) {
            o4_data = Some(data);
        }
        let o4_data = o4_data.or_else(|| o4_data_latest.take());
        o4_data_latest.clone_from(&o4_data);

        let mut o5_data = None;
        while let Some((_, data)) = o5.next_if(|(index, _)| index <= &max_index) {
            o5_data = Some(data);
        }
        let o5_data = o5_data.or_else(|| o5_data_latest.take());
        o5_data_latest.clone_from(&o5_data);

        Some((
            max_index, r0_data, r1_data, o0_data, o1_data, o2_data, o3_data, o4_data, o5_data,
        ))
    }
}

/// Returns a new [`RangeZip2x7`] iterator.
///
/// The number of elements in a range zip iterator corresponds to the number of elements in the
/// shortest of its required iterators (`r0`, `r1`).
///
/// Each call to `next` is guaranteed to yield the next value for each required iterator,
/// as well as the most recent index amongst all of them.
///
/// Optional iterators accumulate their state and yield their most recent value (if any),
/// each time the required iterators fire.
pub fn range_zip_2x7<
    Idx,
    IR0,
    R0,
    IR1,
    R1,
    IO0,
    O0,
    IO1,
    O1,
    IO2,
    O2,
    IO3,
    O3,
    IO4,
    O4,
    IO5,
    O5,
    IO6,
    O6,
>(
    r0: IR0,
    r1: IR1,
    o0: IO0,
    o1: IO1,
    o2: IO2,
    o3: IO3,
    o4: IO4,
    o5: IO5,
    o6: IO6,
) -> RangeZip2x7<
    Idx,
    IR0::IntoIter,
    R0,
    IR1::IntoIter,
    R1,
    IO0::IntoIter,
    O0,
    IO1::IntoIter,
    O1,
    IO2::IntoIter,
    O2,
    IO3::IntoIter,
    O3,
    IO4::IntoIter,
    O4,
    IO5::IntoIter,
    O5,
    IO6::IntoIter,
    O6,
>
where
    Idx: std::cmp::Ord,
    IR0: IntoIterator<Item = (Idx, R0)>,
    IR1: IntoIterator<Item = (Idx, R1)>,
    IO0: IntoIterator<Item = (Idx, O0)>,
    IO1: IntoIterator<Item = (Idx, O1)>,
    IO2: IntoIterator<Item = (Idx, O2)>,
    IO3: IntoIterator<Item = (Idx, O3)>,
    IO4: IntoIterator<Item = (Idx, O4)>,
    IO5: IntoIterator<Item = (Idx, O5)>,
    IO6: IntoIterator<Item = (Idx, O6)>,
{
    RangeZip2x7 {
        r0: r0.into_iter(),
        r1: r1.into_iter(),
        o0: o0.into_iter().peekable(),
        o1: o1.into_iter().peekable(),
        o2: o2.into_iter().peekable(),
        o3: o3.into_iter().peekable(),
        o4: o4.into_iter().peekable(),
        o5: o5.into_iter().peekable(),
        o6: o6.into_iter().peekable(),

        o0_data_latest: None,
        o1_data_latest: None,
        o2_data_latest: None,
        o3_data_latest: None,
        o4_data_latest: None,
        o5_data_latest: None,
        o6_data_latest: None,
    }
}

/// Implements a range zip iterator combinator with 2 required iterators and 2 optional
/// iterators.
///
/// See [`range_zip_2x7`] for more information.
pub struct RangeZip2x7<
    Idx,
    IR0,
    R0,
    IR1,
    R1,
    IO0,
    O0,
    IO1,
    O1,
    IO2,
    O2,
    IO3,
    O3,
    IO4,
    O4,
    IO5,
    O5,
    IO6,
    O6,
> where
    Idx: std::cmp::Ord,
    IR0: Iterator<Item = (Idx, R0)>,
    IR1: Iterator<Item = (Idx, R1)>,
    IO0: Iterator<Item = (Idx, O0)>,
    IO1: Iterator<Item = (Idx, O1)>,
    IO2: Iterator<Item = (Idx, O2)>,
    IO3: Iterator<Item = (Idx, O3)>,
    IO4: Iterator<Item = (Idx, O4)>,
    IO5: Iterator<Item = (Idx, O5)>,
    IO6: Iterator<Item = (Idx, O6)>,
{
    r0: IR0,
    r1: IR1,
    o0: Peekable<IO0>,
    o1: Peekable<IO1>,
    o2: Peekable<IO2>,
    o3: Peekable<IO3>,
    o4: Peekable<IO4>,
    o5: Peekable<IO5>,
    o6: Peekable<IO6>,

    o0_data_latest: Option<O0>,
    o1_data_latest: Option<O1>,
    o2_data_latest: Option<O2>,
    o3_data_latest: Option<O3>,
    o4_data_latest: Option<O4>,
    o5_data_latest: Option<O5>,
    o6_data_latest: Option<O6>,
}

impl<Idx, IR0, R0, IR1, R1, IO0, O0, IO1, O1, IO2, O2, IO3, O3, IO4, O4, IO5, O5, IO6, O6> Iterator
    for RangeZip2x7<
        Idx,
        IR0,
        R0,
        IR1,
        R1,
        IO0,
        O0,
        IO1,
        O1,
        IO2,
        O2,
        IO3,
        O3,
        IO4,
        O4,
        IO5,
        O5,
        IO6,
        O6,
    >
where
    Idx: std::cmp::Ord,
    IR0: Iterator<Item = (Idx, R0)>,
    IR1: Iterator<Item = (Idx, R1)>,
    IO0: Iterator<Item = (Idx, O0)>,
    IO1: Iterator<Item = (Idx, O1)>,
    IO2: Iterator<Item = (Idx, O2)>,
    IO3: Iterator<Item = (Idx, O3)>,
    IO4: Iterator<Item = (Idx, O4)>,
    IO5: Iterator<Item = (Idx, O5)>,
    IO6: Iterator<Item = (Idx, O6)>,
    O0: Clone,
    O1: Clone,
    O2: Clone,
    O3: Clone,
    O4: Clone,
    O5: Clone,
    O6: Clone,
{
    type Item = (
        Idx,
        R0,
        R1,
        Option<O0>,
        Option<O1>,
        Option<O2>,
        Option<O3>,
        Option<O4>,
        Option<O5>,
        Option<O6>,
    );

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let Self {
            r0,
            r1,
            o0,
            o1,
            o2,
            o3,
            o4,
            o5,
            o6,
            o0_data_latest,
            o1_data_latest,
            o2_data_latest,
            o3_data_latest,
            o4_data_latest,
            o5_data_latest,
            o6_data_latest,
        } = self;

        let (r0_index, r0_data) = r0.next()?;
        let (r1_index, r1_data) = r1.next()?;

        let max_index = [r0_index, r1_index].into_iter().max()?;

        let mut o0_data = None;
        while let Some((_, data)) = o0.next_if(|(index, _)| index <= &max_index) {
            o0_data = Some(data);
        }
        let o0_data = o0_data.or_else(|| o0_data_latest.take());
        o0_data_latest.clone_from(&o0_data);

        let mut o1_data = None;
        while let Some((_, data)) = o1.next_if(|(index, _)| index <= &max_index) {
            o1_data = Some(data);
        }
        let o1_data = o1_data.or_else(|| o1_data_latest.take());
        o1_data_latest.clone_from(&o1_data);

        let mut o2_data = None;
        while let Some((_, data)) = o2.next_if(|(index, _)| index <= &max_index) {
            o2_data = Some(data);
        }
        let o2_data = o2_data.or_else(|| o2_data_latest.take());
        o2_data_latest.clone_from(&o2_data);

        let mut o3_data = None;
        while let Some((_, data)) = o3.next_if(|(index, _)| index <= &max_index) {
            o3_data = Some(data);
        }
        let o3_data = o3_data.or_else(|| o3_data_latest.take());
        o3_data_latest.clone_from(&o3_data);

        let mut o4_data = None;
        while let Some((_, data)) = o4.next_if(|(index, _)| index <= &max_index) {
            o4_data = Some(data);
        }
        let o4_data = o4_data.or_else(|| o4_data_latest.take());
        o4_data_latest.clone_from(&o4_data);

        let mut o5_data = None;
        while let Some((_, data)) = o5.next_if(|(index, _)| index <= &max_index) {
            o5_data = Some(data);
        }
        let o5_data = o5_data.or_else(|| o5_data_latest.take());
        o5_data_latest.clone_from(&o5_data);

        let mut o6_data = None;
        while let Some((_, data)) = o6.next_if(|(index, _)| index <= &max_index) {
            o6_data = Some(data);
        }
        let o6_data = o6_data.or_else(|| o6_data_latest.take());
        o6_data_latest.clone_from(&o6_data);

        Some((
            max_index, r0_data, r1_data, o0_data, o1_data, o2_data, o3_data, o4_data, o5_data,
            o6_data,
        ))
    }
}

/// Returns a new [`RangeZip2x8`] iterator.
///
/// The number of elements in a range zip iterator corresponds to the number of elements in the
/// shortest of its required iterators (`r0`, `r1`).
///
/// Each call to `next` is guaranteed to yield the next value for each required iterator,
/// as well as the most recent index amongst all of them.
///
/// Optional iterators accumulate their state and yield their most recent value (if any),
/// each time the required iterators fire.
pub fn range_zip_2x8<
    Idx,
    IR0,
    R0,
    IR1,
    R1,
    IO0,
    O0,
    IO1,
    O1,
    IO2,
    O2,
    IO3,
    O3,
    IO4,
    O4,
    IO5,
    O5,
    IO6,
    O6,
    IO7,
    O7,
>(
    r0: IR0,
    r1: IR1,
    o0: IO0,
    o1: IO1,
    o2: IO2,
    o3: IO3,
    o4: IO4,
    o5: IO5,
    o6: IO6,
    o7: IO7,
) -> RangeZip2x8<
    Idx,
    IR0::IntoIter,
    R0,
    IR1::IntoIter,
    R1,
    IO0::IntoIter,
    O0,
    IO1::IntoIter,
    O1,
    IO2::IntoIter,
    O2,
    IO3::IntoIter,
    O3,
    IO4::IntoIter,
    O4,
    IO5::IntoIter,
    O5,
    IO6::IntoIter,
    O6,
    IO7::IntoIter,
    O7,
>
where
    Idx: std::cmp::Ord,
    IR0: IntoIterator<Item = (Idx, R0)>,
    IR1: IntoIterator<Item = (Idx, R1)>,
    IO0: IntoIterator<Item = (Idx, O0)>,
    IO1: IntoIterator<Item = (Idx, O1)>,
    IO2: IntoIterator<Item = (Idx, O2)>,
    IO3: IntoIterator<Item = (Idx, O3)>,
    IO4: IntoIterator<Item = (Idx, O4)>,
    IO5: IntoIterator<Item = (Idx, O5)>,
    IO6: IntoIterator<Item = (Idx, O6)>,
    IO7: IntoIterator<Item = (Idx, O7)>,
{
    RangeZip2x8 {
        r0: r0.into_iter(),
        r1: r1.into_iter(),
        o0: o0.into_iter().peekable(),
        o1: o1.into_iter().peekable(),
        o2: o2.into_iter().peekable(),
        o3: o3.into_iter().peekable(),
        o4: o4.into_iter().peekable(),
        o5: o5.into_iter().peekable(),
        o6: o6.into_iter().peekable(),
        o7: o7.into_iter().peekable(),

        o0_data_latest: None,
        o1_data_latest: None,
        o2_data_latest: None,
        o3_data_latest: None,
        o4_data_latest: None,
        o5_data_latest: None,
        o6_data_latest: None,
        o7_data_latest: None,
    }
}

/// Implements a range zip iterator combinator with 2 required iterators and 2 optional
/// iterators.
///
/// See [`range_zip_2x8`] for more information.
pub struct RangeZip2x8<
    Idx,
    IR0,
    R0,
    IR1,
    R1,
    IO0,
    O0,
    IO1,
    O1,
    IO2,
    O2,
    IO3,
    O3,
    IO4,
    O4,
    IO5,
    O5,
    IO6,
    O6,
    IO7,
    O7,
> where
    Idx: std::cmp::Ord,
    IR0: Iterator<Item = (Idx, R0)>,
    IR1: Iterator<Item = (Idx, R1)>,
    IO0: Iterator<Item = (Idx, O0)>,
    IO1: Iterator<Item = (Idx, O1)>,
    IO2: Iterator<Item = (Idx, O2)>,
    IO3: Iterator<Item = (Idx, O3)>,
    IO4: Iterator<Item = (Idx, O4)>,
    IO5: Iterator<Item = (Idx, O5)>,
    IO6: Iterator<Item = (Idx, O6)>,
    IO7: Iterator<Item = (Idx, O7)>,
{
    r0: IR0,
    r1: IR1,
    o0: Peekable<IO0>,
    o1: Peekable<IO1>,
    o2: Peekable<IO2>,
    o3: Peekable<IO3>,
    o4: Peekable<IO4>,
    o5: Peekable<IO5>,
    o6: Peekable<IO6>,
    o7: Peekable<IO7>,

    o0_data_latest: Option<O0>,
    o1_data_latest: Option<O1>,
    o2_data_latest: Option<O2>,
    o3_data_latest: Option<O3>,
    o4_data_latest: Option<O4>,
    o5_data_latest: Option<O5>,
    o6_data_latest: Option<O6>,
    o7_data_latest: Option<O7>,
}

impl<Idx, IR0, R0, IR1, R1, IO0, O0, IO1, O1, IO2, O2, IO3, O3, IO4, O4, IO5, O5, IO6, O6, IO7, O7>
    Iterator
    for RangeZip2x8<
        Idx,
        IR0,
        R0,
        IR1,
        R1,
        IO0,
        O0,
        IO1,
        O1,
        IO2,
        O2,
        IO3,
        O3,
        IO4,
        O4,
        IO5,
        O5,
        IO6,
        O6,
        IO7,
        O7,
    >
where
    Idx: std::cmp::Ord,
    IR0: Iterator<Item = (Idx, R0)>,
    IR1: Iterator<Item = (Idx, R1)>,
    IO0: Iterator<Item = (Idx, O0)>,
    IO1: Iterator<Item = (Idx, O1)>,
    IO2: Iterator<Item = (Idx, O2)>,
    IO3: Iterator<Item = (Idx, O3)>,
    IO4: Iterator<Item = (Idx, O4)>,
    IO5: Iterator<Item = (Idx, O5)>,
    IO6: Iterator<Item = (Idx, O6)>,
    IO7: Iterator<Item = (Idx, O7)>,
    O0: Clone,
    O1: Clone,
    O2: Clone,
    O3: Clone,
    O4: Clone,
    O5: Clone,
    O6: Clone,
    O7: Clone,
{
    type Item = (
        Idx,
        R0,
        R1,
        Option<O0>,
        Option<O1>,
        Option<O2>,
        Option<O3>,
        Option<O4>,
        Option<O5>,
        Option<O6>,
        Option<O7>,
    );

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let Self {
            r0,
            r1,
            o0,
            o1,
            o2,
            o3,
            o4,
            o5,
            o6,
            o7,
            o0_data_latest,
            o1_data_latest,
            o2_data_latest,
            o3_data_latest,
            o4_data_latest,
            o5_data_latest,
            o6_data_latest,
            o7_data_latest,
        } = self;

        let (r0_index, r0_data) = r0.next()?;
        let (r1_index, r1_data) = r1.next()?;

        let max_index = [r0_index, r1_index].into_iter().max()?;

        let mut o0_data = None;
        while let Some((_, data)) = o0.next_if(|(index, _)| index <= &max_index) {
            o0_data = Some(data);
        }
        let o0_data = o0_data.or_else(|| o0_data_latest.take());
        o0_data_latest.clone_from(&o0_data);

        let mut o1_data = None;
        while let Some((_, data)) = o1.next_if(|(index, _)| index <= &max_index) {
            o1_data = Some(data);
        }
        let o1_data = o1_data.or_else(|| o1_data_latest.take());
        o1_data_latest.clone_from(&o1_data);

        let mut o2_data = None;
        while let Some((_, data)) = o2.next_if(|(index, _)| index <= &max_index) {
            o2_data = Some(data);
        }
        let o2_data = o2_data.or_else(|| o2_data_latest.take());
        o2_data_latest.clone_from(&o2_data);

        let mut o3_data = None;
        while let Some((_, data)) = o3.next_if(|(index, _)| index <= &max_index) {
            o3_data = Some(data);
        }
        let o3_data = o3_data.or_else(|| o3_data_latest.take());
        o3_data_latest.clone_from(&o3_data);

        let mut o4_data = None;
        while let Some((_, data)) = o4.next_if(|(index, _)| index <= &max_index) {
            o4_data = Some(data);
        }
        let o4_data = o4_data.or_else(|| o4_data_latest.take());
        o4_data_latest.clone_from(&o4_data);

        let mut o5_data = None;
        while let Some((_, data)) = o5.next_if(|(index, _)| index <= &max_index) {
            o5_data = Some(data);
        }
        let o5_data = o5_data.or_else(|| o5_data_latest.take());
        o5_data_latest.clone_from(&o5_data);

        let mut o6_data = None;
        while let Some((_, data)) = o6.next_if(|(index, _)| index <= &max_index) {
            o6_data = Some(data);
        }
        let o6_data = o6_data.or_else(|| o6_data_latest.take());
        o6_data_latest.clone_from(&o6_data);

        let mut o7_data = None;
        while let Some((_, data)) = o7.next_if(|(index, _)| index <= &max_index) {
            o7_data = Some(data);
        }
        let o7_data = o7_data.or_else(|| o7_data_latest.take());
        o7_data_latest.clone_from(&o7_data);

        Some((
            max_index, r0_data, r1_data, o0_data, o1_data, o2_data, o3_data, o4_data, o5_data,
            o6_data, o7_data,
        ))
    }
}

/// Returns a new [`RangeZip2x9`] iterator.
///
/// The number of elements in a range zip iterator corresponds to the number of elements in the
/// shortest of its required iterators (`r0`, `r1`).
///
/// Each call to `next` is guaranteed to yield the next value for each required iterator,
/// as well as the most recent index amongst all of them.
///
/// Optional iterators accumulate their state and yield their most recent value (if any),
/// each time the required iterators fire.
pub fn range_zip_2x9<
    Idx,
    IR0,
    R0,
    IR1,
    R1,
    IO0,
    O0,
    IO1,
    O1,
    IO2,
    O2,
    IO3,
    O3,
    IO4,
    O4,
    IO5,
    O5,
    IO6,
    O6,
    IO7,
    O7,
    IO8,
    O8,
>(
    r0: IR0,
    r1: IR1,
    o0: IO0,
    o1: IO1,
    o2: IO2,
    o3: IO3,
    o4: IO4,
    o5: IO5,
    o6: IO6,
    o7: IO7,
    o8: IO8,
) -> RangeZip2x9<
    Idx,
    IR0::IntoIter,
    R0,
    IR1::IntoIter,
    R1,
    IO0::IntoIter,
    O0,
    IO1::IntoIter,
    O1,
    IO2::IntoIter,
    O2,
    IO3::IntoIter,
    O3,
    IO4::IntoIter,
    O4,
    IO5::IntoIter,
    O5,
    IO6::IntoIter,
    O6,
    IO7::IntoIter,
    O7,
    IO8::IntoIter,
    O8,
>
where
    Idx: std::cmp::Ord,
    IR0: IntoIterator<Item = (Idx, R0)>,
    IR1: IntoIterator<Item = (Idx, R1)>,
    IO0: IntoIterator<Item = (Idx, O0)>,
    IO1: IntoIterator<Item = (Idx, O1)>,
    IO2: IntoIterator<Item = (Idx, O2)>,
    IO3: IntoIterator<Item = (Idx, O3)>,
    IO4: IntoIterator<Item = (Idx, O4)>,
    IO5: IntoIterator<Item = (Idx, O5)>,
    IO6: IntoIterator<Item = (Idx, O6)>,
    IO7: IntoIterator<Item = (Idx, O7)>,
    IO8: IntoIterator<Item = (Idx, O8)>,
{
    RangeZip2x9 {
        r0: r0.into_iter(),
        r1: r1.into_iter(),
        o0: o0.into_iter().peekable(),
        o1: o1.into_iter().peekable(),
        o2: o2.into_iter().peekable(),
        o3: o3.into_iter().peekable(),
        o4: o4.into_iter().peekable(),
        o5: o5.into_iter().peekable(),
        o6: o6.into_iter().peekable(),
        o7: o7.into_iter().peekable(),
        o8: o8.into_iter().peekable(),

        o0_data_latest: None,
        o1_data_latest: None,
        o2_data_latest: None,
        o3_data_latest: None,
        o4_data_latest: None,
        o5_data_latest: None,
        o6_data_latest: None,
        o7_data_latest: None,
        o8_data_latest: None,
    }
}

/// Implements a range zip iterator combinator with 2 required iterators and 2 optional
/// iterators.
///
/// See [`range_zip_2x9`] for more information.
pub struct RangeZip2x9<
    Idx,
    IR0,
    R0,
    IR1,
    R1,
    IO0,
    O0,
    IO1,
    O1,
    IO2,
    O2,
    IO3,
    O3,
    IO4,
    O4,
    IO5,
    O5,
    IO6,
    O6,
    IO7,
    O7,
    IO8,
    O8,
> where
    Idx: std::cmp::Ord,
    IR0: Iterator<Item = (Idx, R0)>,
    IR1: Iterator<Item = (Idx, R1)>,
    IO0: Iterator<Item = (Idx, O0)>,
    IO1: Iterator<Item = (Idx, O1)>,
    IO2: Iterator<Item = (Idx, O2)>,
    IO3: Iterator<Item = (Idx, O3)>,
    IO4: Iterator<Item = (Idx, O4)>,
    IO5: Iterator<Item = (Idx, O5)>,
    IO6: Iterator<Item = (Idx, O6)>,
    IO7: Iterator<Item = (Idx, O7)>,
    IO8: Iterator<Item = (Idx, O8)>,
{
    r0: IR0,
    r1: IR1,
    o0: Peekable<IO0>,
    o1: Peekable<IO1>,
    o2: Peekable<IO2>,
    o3: Peekable<IO3>,
    o4: Peekable<IO4>,
    o5: Peekable<IO5>,
    o6: Peekable<IO6>,
    o7: Peekable<IO7>,
    o8: Peekable<IO8>,

    o0_data_latest: Option<O0>,
    o1_data_latest: Option<O1>,
    o2_data_latest: Option<O2>,
    o3_data_latest: Option<O3>,
    o4_data_latest: Option<O4>,
    o5_data_latest: Option<O5>,
    o6_data_latest: Option<O6>,
    o7_data_latest: Option<O7>,
    o8_data_latest: Option<O8>,
}

impl<
    Idx,
    IR0,
    R0,
    IR1,
    R1,
    IO0,
    O0,
    IO1,
    O1,
    IO2,
    O2,
    IO3,
    O3,
    IO4,
    O4,
    IO5,
    O5,
    IO6,
    O6,
    IO7,
    O7,
    IO8,
    O8,
> Iterator
    for RangeZip2x9<
        Idx,
        IR0,
        R0,
        IR1,
        R1,
        IO0,
        O0,
        IO1,
        O1,
        IO2,
        O2,
        IO3,
        O3,
        IO4,
        O4,
        IO5,
        O5,
        IO6,
        O6,
        IO7,
        O7,
        IO8,
        O8,
    >
where
    Idx: std::cmp::Ord,
    IR0: Iterator<Item = (Idx, R0)>,
    IR1: Iterator<Item = (Idx, R1)>,
    IO0: Iterator<Item = (Idx, O0)>,
    IO1: Iterator<Item = (Idx, O1)>,
    IO2: Iterator<Item = (Idx, O2)>,
    IO3: Iterator<Item = (Idx, O3)>,
    IO4: Iterator<Item = (Idx, O4)>,
    IO5: Iterator<Item = (Idx, O5)>,
    IO6: Iterator<Item = (Idx, O6)>,
    IO7: Iterator<Item = (Idx, O7)>,
    IO8: Iterator<Item = (Idx, O8)>,
    O0: Clone,
    O1: Clone,
    O2: Clone,
    O3: Clone,
    O4: Clone,
    O5: Clone,
    O6: Clone,
    O7: Clone,
    O8: Clone,
{
    type Item = (
        Idx,
        R0,
        R1,
        Option<O0>,
        Option<O1>,
        Option<O2>,
        Option<O3>,
        Option<O4>,
        Option<O5>,
        Option<O6>,
        Option<O7>,
        Option<O8>,
    );

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let Self {
            r0,
            r1,
            o0,
            o1,
            o2,
            o3,
            o4,
            o5,
            o6,
            o7,
            o8,
            o0_data_latest,
            o1_data_latest,
            o2_data_latest,
            o3_data_latest,
            o4_data_latest,
            o5_data_latest,
            o6_data_latest,
            o7_data_latest,
            o8_data_latest,
        } = self;

        let (r0_index, r0_data) = r0.next()?;
        let (r1_index, r1_data) = r1.next()?;

        let max_index = [r0_index, r1_index].into_iter().max()?;

        let mut o0_data = None;
        while let Some((_, data)) = o0.next_if(|(index, _)| index <= &max_index) {
            o0_data = Some(data);
        }
        let o0_data = o0_data.or_else(|| o0_data_latest.take());
        o0_data_latest.clone_from(&o0_data);

        let mut o1_data = None;
        while let Some((_, data)) = o1.next_if(|(index, _)| index <= &max_index) {
            o1_data = Some(data);
        }
        let o1_data = o1_data.or_else(|| o1_data_latest.take());
        o1_data_latest.clone_from(&o1_data);

        let mut o2_data = None;
        while let Some((_, data)) = o2.next_if(|(index, _)| index <= &max_index) {
            o2_data = Some(data);
        }
        let o2_data = o2_data.or_else(|| o2_data_latest.take());
        o2_data_latest.clone_from(&o2_data);

        let mut o3_data = None;
        while let Some((_, data)) = o3.next_if(|(index, _)| index <= &max_index) {
            o3_data = Some(data);
        }
        let o3_data = o3_data.or_else(|| o3_data_latest.take());
        o3_data_latest.clone_from(&o3_data);

        let mut o4_data = None;
        while let Some((_, data)) = o4.next_if(|(index, _)| index <= &max_index) {
            o4_data = Some(data);
        }
        let o4_data = o4_data.or_else(|| o4_data_latest.take());
        o4_data_latest.clone_from(&o4_data);

        let mut o5_data = None;
        while let Some((_, data)) = o5.next_if(|(index, _)| index <= &max_index) {
            o5_data = Some(data);
        }
        let o5_data = o5_data.or_else(|| o5_data_latest.take());
        o5_data_latest.clone_from(&o5_data);

        let mut o6_data = None;
        while let Some((_, data)) = o6.next_if(|(index, _)| index <= &max_index) {
            o6_data = Some(data);
        }
        let o6_data = o6_data.or_else(|| o6_data_latest.take());
        o6_data_latest.clone_from(&o6_data);

        let mut o7_data = None;
        while let Some((_, data)) = o7.next_if(|(index, _)| index <= &max_index) {
            o7_data = Some(data);
        }
        let o7_data = o7_data.or_else(|| o7_data_latest.take());
        o7_data_latest.clone_from(&o7_data);

        let mut o8_data = None;
        while let Some((_, data)) = o8.next_if(|(index, _)| index <= &max_index) {
            o8_data = Some(data);
        }
        let o8_data = o8_data.or_else(|| o8_data_latest.take());
        o8_data_latest.clone_from(&o8_data);

        Some((
            max_index, r0_data, r1_data, o0_data, o1_data, o2_data, o3_data, o4_data, o5_data,
            o6_data, o7_data, o8_data,
        ))
    }
}
