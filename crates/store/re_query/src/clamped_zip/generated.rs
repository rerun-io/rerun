// This file was generated using `cargo r -p re_query --all-features --bin clamped_zip`.
// DO NOT EDIT.

// ---

#![expect(clippy::too_many_arguments)]
#![expect(clippy::type_complexity)]

/// Returns a new [`ClampedZip1x1`] iterator.
///
/// The number of elements in a clamped zip iterator corresponds to the number of elements in the
/// shortest of its required iterators (`r0`).
///
/// Optional iterators (`o0`) will repeat their latest values if they happen to be too short
/// to be zipped with the shortest of the required iterators.
///
/// If an optional iterator is not only too short but actually empty, its associated default function
/// (`o0_default_fn`) will be executed and the resulting value repeated as necessary.
pub fn clamped_zip_1x1<R0, O0, D0>(
    r0: R0,
    o0: O0,
    o0_default_fn: D0,
) -> ClampedZip1x1<R0::IntoIter, O0::IntoIter, D0>
where
    R0: IntoIterator,
    O0: IntoIterator,
    O0::Item: Clone,
    D0: Fn() -> O0::Item,
{
    ClampedZip1x1 {
        r0: r0.into_iter(),
        o0: o0.into_iter(),
        o0_default_fn,
        o0_latest_value: None,
    }
}

/// Implements a clamped zip iterator combinator with 2 required iterators and 2 optional
/// iterators.
///
/// See [`clamped_zip_1x1`] for more information.
pub struct ClampedZip1x1<R0, O0, D0>
where
    R0: Iterator,
    O0: Iterator,
    O0::Item: Clone,
    D0: Fn() -> O0::Item,
{
    r0: R0,
    o0: O0,
    o0_default_fn: D0,

    o0_latest_value: Option<O0::Item>,
}

impl<R0, O0, D0> Iterator for ClampedZip1x1<R0, O0, D0>
where
    R0: Iterator,
    O0: Iterator,
    O0::Item: Clone,
    D0: Fn() -> O0::Item,
{
    type Item = (R0::Item, O0::Item);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let r0_next = self.r0.next()?;
        let o0_next = self.o0.next().or_else(|| self.o0_latest_value.take());

        self.o0_latest_value.clone_from(&o0_next);

        Some((r0_next, o0_next.unwrap_or_else(|| (self.o0_default_fn)())))
    }
}

/// Returns a new [`ClampedZip1x2`] iterator.
///
/// The number of elements in a clamped zip iterator corresponds to the number of elements in the
/// shortest of its required iterators (`r0`).
///
/// Optional iterators (`o0`, `o1`) will repeat their latest values if they happen to be too short
/// to be zipped with the shortest of the required iterators.
///
/// If an optional iterator is not only too short but actually empty, its associated default function
/// (`o0_default_fn`, `o1_default_fn`) will be executed and the resulting value repeated as necessary.
pub fn clamped_zip_1x2<R0, O0, O1, D0, D1>(
    r0: R0,
    o0: O0,
    o0_default_fn: D0,
    o1: O1,
    o1_default_fn: D1,
) -> ClampedZip1x2<R0::IntoIter, O0::IntoIter, O1::IntoIter, D0, D1>
where
    R0: IntoIterator,
    O0: IntoIterator,
    O0::Item: Clone,
    O1: IntoIterator,
    O1::Item: Clone,
    D0: Fn() -> O0::Item,
    D1: Fn() -> O1::Item,
{
    ClampedZip1x2 {
        r0: r0.into_iter(),
        o0: o0.into_iter(),
        o1: o1.into_iter(),
        o0_default_fn,
        o1_default_fn,
        o0_latest_value: None,
        o1_latest_value: None,
    }
}

/// Implements a clamped zip iterator combinator with 2 required iterators and 2 optional
/// iterators.
///
/// See [`clamped_zip_1x2`] for more information.
pub struct ClampedZip1x2<R0, O0, O1, D0, D1>
where
    R0: Iterator,
    O0: Iterator,
    O0::Item: Clone,
    O1: Iterator,
    O1::Item: Clone,
    D0: Fn() -> O0::Item,
    D1: Fn() -> O1::Item,
{
    r0: R0,
    o0: O0,
    o1: O1,
    o0_default_fn: D0,
    o1_default_fn: D1,

    o0_latest_value: Option<O0::Item>,
    o1_latest_value: Option<O1::Item>,
}

impl<R0, O0, O1, D0, D1> Iterator for ClampedZip1x2<R0, O0, O1, D0, D1>
where
    R0: Iterator,
    O0: Iterator,
    O0::Item: Clone,
    O1: Iterator,
    O1::Item: Clone,
    D0: Fn() -> O0::Item,
    D1: Fn() -> O1::Item,
{
    type Item = (R0::Item, O0::Item, O1::Item);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let r0_next = self.r0.next()?;
        let o0_next = self.o0.next().or_else(|| self.o0_latest_value.take());
        let o1_next = self.o1.next().or_else(|| self.o1_latest_value.take());

        self.o0_latest_value.clone_from(&o0_next);
        self.o1_latest_value.clone_from(&o1_next);

        Some((
            r0_next,
            o0_next.unwrap_or_else(|| (self.o0_default_fn)()),
            o1_next.unwrap_or_else(|| (self.o1_default_fn)()),
        ))
    }
}

/// Returns a new [`ClampedZip1x3`] iterator.
///
/// The number of elements in a clamped zip iterator corresponds to the number of elements in the
/// shortest of its required iterators (`r0`).
///
/// Optional iterators (`o0`, `o1`, `o2`) will repeat their latest values if they happen to be too short
/// to be zipped with the shortest of the required iterators.
///
/// If an optional iterator is not only too short but actually empty, its associated default function
/// (`o0_default_fn`, `o1_default_fn`, `o2_default_fn`) will be executed and the resulting value repeated as necessary.
pub fn clamped_zip_1x3<R0, O0, O1, O2, D0, D1, D2>(
    r0: R0,
    o0: O0,
    o0_default_fn: D0,
    o1: O1,
    o1_default_fn: D1,
    o2: O2,
    o2_default_fn: D2,
) -> ClampedZip1x3<R0::IntoIter, O0::IntoIter, O1::IntoIter, O2::IntoIter, D0, D1, D2>
where
    R0: IntoIterator,
    O0: IntoIterator,
    O0::Item: Clone,
    O1: IntoIterator,
    O1::Item: Clone,
    O2: IntoIterator,
    O2::Item: Clone,
    D0: Fn() -> O0::Item,
    D1: Fn() -> O1::Item,
    D2: Fn() -> O2::Item,
{
    ClampedZip1x3 {
        r0: r0.into_iter(),
        o0: o0.into_iter(),
        o1: o1.into_iter(),
        o2: o2.into_iter(),
        o0_default_fn,
        o1_default_fn,
        o2_default_fn,
        o0_latest_value: None,
        o1_latest_value: None,
        o2_latest_value: None,
    }
}

/// Implements a clamped zip iterator combinator with 2 required iterators and 2 optional
/// iterators.
///
/// See [`clamped_zip_1x3`] for more information.
pub struct ClampedZip1x3<R0, O0, O1, O2, D0, D1, D2>
where
    R0: Iterator,
    O0: Iterator,
    O0::Item: Clone,
    O1: Iterator,
    O1::Item: Clone,
    O2: Iterator,
    O2::Item: Clone,
    D0: Fn() -> O0::Item,
    D1: Fn() -> O1::Item,
    D2: Fn() -> O2::Item,
{
    r0: R0,
    o0: O0,
    o1: O1,
    o2: O2,
    o0_default_fn: D0,
    o1_default_fn: D1,
    o2_default_fn: D2,

    o0_latest_value: Option<O0::Item>,
    o1_latest_value: Option<O1::Item>,
    o2_latest_value: Option<O2::Item>,
}

impl<R0, O0, O1, O2, D0, D1, D2> Iterator for ClampedZip1x3<R0, O0, O1, O2, D0, D1, D2>
where
    R0: Iterator,
    O0: Iterator,
    O0::Item: Clone,
    O1: Iterator,
    O1::Item: Clone,
    O2: Iterator,
    O2::Item: Clone,
    D0: Fn() -> O0::Item,
    D1: Fn() -> O1::Item,
    D2: Fn() -> O2::Item,
{
    type Item = (R0::Item, O0::Item, O1::Item, O2::Item);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let r0_next = self.r0.next()?;
        let o0_next = self.o0.next().or_else(|| self.o0_latest_value.take());
        let o1_next = self.o1.next().or_else(|| self.o1_latest_value.take());
        let o2_next = self.o2.next().or_else(|| self.o2_latest_value.take());

        self.o0_latest_value.clone_from(&o0_next);
        self.o1_latest_value.clone_from(&o1_next);
        self.o2_latest_value.clone_from(&o2_next);

        Some((
            r0_next,
            o0_next.unwrap_or_else(|| (self.o0_default_fn)()),
            o1_next.unwrap_or_else(|| (self.o1_default_fn)()),
            o2_next.unwrap_or_else(|| (self.o2_default_fn)()),
        ))
    }
}

/// Returns a new [`ClampedZip1x4`] iterator.
///
/// The number of elements in a clamped zip iterator corresponds to the number of elements in the
/// shortest of its required iterators (`r0`).
///
/// Optional iterators (`o0`, `o1`, `o2`, `o3`) will repeat their latest values if they happen to be too short
/// to be zipped with the shortest of the required iterators.
///
/// If an optional iterator is not only too short but actually empty, its associated default function
/// (`o0_default_fn`, `o1_default_fn`, `o2_default_fn`, `o3_default_fn`) will be executed and the resulting value repeated as necessary.
pub fn clamped_zip_1x4<R0, O0, O1, O2, O3, D0, D1, D2, D3>(
    r0: R0,
    o0: O0,
    o0_default_fn: D0,
    o1: O1,
    o1_default_fn: D1,
    o2: O2,
    o2_default_fn: D2,
    o3: O3,
    o3_default_fn: D3,
) -> ClampedZip1x4<
    R0::IntoIter,
    O0::IntoIter,
    O1::IntoIter,
    O2::IntoIter,
    O3::IntoIter,
    D0,
    D1,
    D2,
    D3,
>
where
    R0: IntoIterator,
    O0: IntoIterator,
    O0::Item: Clone,
    O1: IntoIterator,
    O1::Item: Clone,
    O2: IntoIterator,
    O2::Item: Clone,
    O3: IntoIterator,
    O3::Item: Clone,
    D0: Fn() -> O0::Item,
    D1: Fn() -> O1::Item,
    D2: Fn() -> O2::Item,
    D3: Fn() -> O3::Item,
{
    ClampedZip1x4 {
        r0: r0.into_iter(),
        o0: o0.into_iter(),
        o1: o1.into_iter(),
        o2: o2.into_iter(),
        o3: o3.into_iter(),
        o0_default_fn,
        o1_default_fn,
        o2_default_fn,
        o3_default_fn,
        o0_latest_value: None,
        o1_latest_value: None,
        o2_latest_value: None,
        o3_latest_value: None,
    }
}

/// Implements a clamped zip iterator combinator with 2 required iterators and 2 optional
/// iterators.
///
/// See [`clamped_zip_1x4`] for more information.
pub struct ClampedZip1x4<R0, O0, O1, O2, O3, D0, D1, D2, D3>
where
    R0: Iterator,
    O0: Iterator,
    O0::Item: Clone,
    O1: Iterator,
    O1::Item: Clone,
    O2: Iterator,
    O2::Item: Clone,
    O3: Iterator,
    O3::Item: Clone,
    D0: Fn() -> O0::Item,
    D1: Fn() -> O1::Item,
    D2: Fn() -> O2::Item,
    D3: Fn() -> O3::Item,
{
    r0: R0,
    o0: O0,
    o1: O1,
    o2: O2,
    o3: O3,
    o0_default_fn: D0,
    o1_default_fn: D1,
    o2_default_fn: D2,
    o3_default_fn: D3,

    o0_latest_value: Option<O0::Item>,
    o1_latest_value: Option<O1::Item>,
    o2_latest_value: Option<O2::Item>,
    o3_latest_value: Option<O3::Item>,
}

impl<R0, O0, O1, O2, O3, D0, D1, D2, D3> Iterator
    for ClampedZip1x4<R0, O0, O1, O2, O3, D0, D1, D2, D3>
where
    R0: Iterator,
    O0: Iterator,
    O0::Item: Clone,
    O1: Iterator,
    O1::Item: Clone,
    O2: Iterator,
    O2::Item: Clone,
    O3: Iterator,
    O3::Item: Clone,
    D0: Fn() -> O0::Item,
    D1: Fn() -> O1::Item,
    D2: Fn() -> O2::Item,
    D3: Fn() -> O3::Item,
{
    type Item = (R0::Item, O0::Item, O1::Item, O2::Item, O3::Item);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let r0_next = self.r0.next()?;
        let o0_next = self.o0.next().or_else(|| self.o0_latest_value.take());
        let o1_next = self.o1.next().or_else(|| self.o1_latest_value.take());
        let o2_next = self.o2.next().or_else(|| self.o2_latest_value.take());
        let o3_next = self.o3.next().or_else(|| self.o3_latest_value.take());

        self.o0_latest_value.clone_from(&o0_next);
        self.o1_latest_value.clone_from(&o1_next);
        self.o2_latest_value.clone_from(&o2_next);
        self.o3_latest_value.clone_from(&o3_next);

        Some((
            r0_next,
            o0_next.unwrap_or_else(|| (self.o0_default_fn)()),
            o1_next.unwrap_or_else(|| (self.o1_default_fn)()),
            o2_next.unwrap_or_else(|| (self.o2_default_fn)()),
            o3_next.unwrap_or_else(|| (self.o3_default_fn)()),
        ))
    }
}

/// Returns a new [`ClampedZip1x5`] iterator.
///
/// The number of elements in a clamped zip iterator corresponds to the number of elements in the
/// shortest of its required iterators (`r0`).
///
/// Optional iterators (`o0`, `o1`, `o2`, `o3`, `o4`) will repeat their latest values if they happen to be too short
/// to be zipped with the shortest of the required iterators.
///
/// If an optional iterator is not only too short but actually empty, its associated default function
/// (`o0_default_fn`, `o1_default_fn`, `o2_default_fn`, `o3_default_fn`, `o4_default_fn`) will be executed and the resulting value repeated as necessary.
pub fn clamped_zip_1x5<R0, O0, O1, O2, O3, O4, D0, D1, D2, D3, D4>(
    r0: R0,
    o0: O0,
    o0_default_fn: D0,
    o1: O1,
    o1_default_fn: D1,
    o2: O2,
    o2_default_fn: D2,
    o3: O3,
    o3_default_fn: D3,
    o4: O4,
    o4_default_fn: D4,
) -> ClampedZip1x5<
    R0::IntoIter,
    O0::IntoIter,
    O1::IntoIter,
    O2::IntoIter,
    O3::IntoIter,
    O4::IntoIter,
    D0,
    D1,
    D2,
    D3,
    D4,
>
where
    R0: IntoIterator,
    O0: IntoIterator,
    O0::Item: Clone,
    O1: IntoIterator,
    O1::Item: Clone,
    O2: IntoIterator,
    O2::Item: Clone,
    O3: IntoIterator,
    O3::Item: Clone,
    O4: IntoIterator,
    O4::Item: Clone,
    D0: Fn() -> O0::Item,
    D1: Fn() -> O1::Item,
    D2: Fn() -> O2::Item,
    D3: Fn() -> O3::Item,
    D4: Fn() -> O4::Item,
{
    ClampedZip1x5 {
        r0: r0.into_iter(),
        o0: o0.into_iter(),
        o1: o1.into_iter(),
        o2: o2.into_iter(),
        o3: o3.into_iter(),
        o4: o4.into_iter(),
        o0_default_fn,
        o1_default_fn,
        o2_default_fn,
        o3_default_fn,
        o4_default_fn,
        o0_latest_value: None,
        o1_latest_value: None,
        o2_latest_value: None,
        o3_latest_value: None,
        o4_latest_value: None,
    }
}

/// Implements a clamped zip iterator combinator with 2 required iterators and 2 optional
/// iterators.
///
/// See [`clamped_zip_1x5`] for more information.
pub struct ClampedZip1x5<R0, O0, O1, O2, O3, O4, D0, D1, D2, D3, D4>
where
    R0: Iterator,
    O0: Iterator,
    O0::Item: Clone,
    O1: Iterator,
    O1::Item: Clone,
    O2: Iterator,
    O2::Item: Clone,
    O3: Iterator,
    O3::Item: Clone,
    O4: Iterator,
    O4::Item: Clone,
    D0: Fn() -> O0::Item,
    D1: Fn() -> O1::Item,
    D2: Fn() -> O2::Item,
    D3: Fn() -> O3::Item,
    D4: Fn() -> O4::Item,
{
    r0: R0,
    o0: O0,
    o1: O1,
    o2: O2,
    o3: O3,
    o4: O4,
    o0_default_fn: D0,
    o1_default_fn: D1,
    o2_default_fn: D2,
    o3_default_fn: D3,
    o4_default_fn: D4,

    o0_latest_value: Option<O0::Item>,
    o1_latest_value: Option<O1::Item>,
    o2_latest_value: Option<O2::Item>,
    o3_latest_value: Option<O3::Item>,
    o4_latest_value: Option<O4::Item>,
}

impl<R0, O0, O1, O2, O3, O4, D0, D1, D2, D3, D4> Iterator
    for ClampedZip1x5<R0, O0, O1, O2, O3, O4, D0, D1, D2, D3, D4>
where
    R0: Iterator,
    O0: Iterator,
    O0::Item: Clone,
    O1: Iterator,
    O1::Item: Clone,
    O2: Iterator,
    O2::Item: Clone,
    O3: Iterator,
    O3::Item: Clone,
    O4: Iterator,
    O4::Item: Clone,
    D0: Fn() -> O0::Item,
    D1: Fn() -> O1::Item,
    D2: Fn() -> O2::Item,
    D3: Fn() -> O3::Item,
    D4: Fn() -> O4::Item,
{
    type Item = (R0::Item, O0::Item, O1::Item, O2::Item, O3::Item, O4::Item);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let r0_next = self.r0.next()?;
        let o0_next = self.o0.next().or_else(|| self.o0_latest_value.take());
        let o1_next = self.o1.next().or_else(|| self.o1_latest_value.take());
        let o2_next = self.o2.next().or_else(|| self.o2_latest_value.take());
        let o3_next = self.o3.next().or_else(|| self.o3_latest_value.take());
        let o4_next = self.o4.next().or_else(|| self.o4_latest_value.take());

        self.o0_latest_value.clone_from(&o0_next);
        self.o1_latest_value.clone_from(&o1_next);
        self.o2_latest_value.clone_from(&o2_next);
        self.o3_latest_value.clone_from(&o3_next);
        self.o4_latest_value.clone_from(&o4_next);

        Some((
            r0_next,
            o0_next.unwrap_or_else(|| (self.o0_default_fn)()),
            o1_next.unwrap_or_else(|| (self.o1_default_fn)()),
            o2_next.unwrap_or_else(|| (self.o2_default_fn)()),
            o3_next.unwrap_or_else(|| (self.o3_default_fn)()),
            o4_next.unwrap_or_else(|| (self.o4_default_fn)()),
        ))
    }
}

/// Returns a new [`ClampedZip1x6`] iterator.
///
/// The number of elements in a clamped zip iterator corresponds to the number of elements in the
/// shortest of its required iterators (`r0`).
///
/// Optional iterators (`o0`, `o1`, `o2`, `o3`, `o4`, `o5`) will repeat their latest values if they happen to be too short
/// to be zipped with the shortest of the required iterators.
///
/// If an optional iterator is not only too short but actually empty, its associated default function
/// (`o0_default_fn`, `o1_default_fn`, `o2_default_fn`, `o3_default_fn`, `o4_default_fn`, `o5_default_fn`) will be executed and the resulting value repeated as necessary.
pub fn clamped_zip_1x6<R0, O0, O1, O2, O3, O4, O5, D0, D1, D2, D3, D4, D5>(
    r0: R0,
    o0: O0,
    o0_default_fn: D0,
    o1: O1,
    o1_default_fn: D1,
    o2: O2,
    o2_default_fn: D2,
    o3: O3,
    o3_default_fn: D3,
    o4: O4,
    o4_default_fn: D4,
    o5: O5,
    o5_default_fn: D5,
) -> ClampedZip1x6<
    R0::IntoIter,
    O0::IntoIter,
    O1::IntoIter,
    O2::IntoIter,
    O3::IntoIter,
    O4::IntoIter,
    O5::IntoIter,
    D0,
    D1,
    D2,
    D3,
    D4,
    D5,
>
where
    R0: IntoIterator,
    O0: IntoIterator,
    O0::Item: Clone,
    O1: IntoIterator,
    O1::Item: Clone,
    O2: IntoIterator,
    O2::Item: Clone,
    O3: IntoIterator,
    O3::Item: Clone,
    O4: IntoIterator,
    O4::Item: Clone,
    O5: IntoIterator,
    O5::Item: Clone,
    D0: Fn() -> O0::Item,
    D1: Fn() -> O1::Item,
    D2: Fn() -> O2::Item,
    D3: Fn() -> O3::Item,
    D4: Fn() -> O4::Item,
    D5: Fn() -> O5::Item,
{
    ClampedZip1x6 {
        r0: r0.into_iter(),
        o0: o0.into_iter(),
        o1: o1.into_iter(),
        o2: o2.into_iter(),
        o3: o3.into_iter(),
        o4: o4.into_iter(),
        o5: o5.into_iter(),
        o0_default_fn,
        o1_default_fn,
        o2_default_fn,
        o3_default_fn,
        o4_default_fn,
        o5_default_fn,
        o0_latest_value: None,
        o1_latest_value: None,
        o2_latest_value: None,
        o3_latest_value: None,
        o4_latest_value: None,
        o5_latest_value: None,
    }
}

/// Implements a clamped zip iterator combinator with 2 required iterators and 2 optional
/// iterators.
///
/// See [`clamped_zip_1x6`] for more information.
pub struct ClampedZip1x6<R0, O0, O1, O2, O3, O4, O5, D0, D1, D2, D3, D4, D5>
where
    R0: Iterator,
    O0: Iterator,
    O0::Item: Clone,
    O1: Iterator,
    O1::Item: Clone,
    O2: Iterator,
    O2::Item: Clone,
    O3: Iterator,
    O3::Item: Clone,
    O4: Iterator,
    O4::Item: Clone,
    O5: Iterator,
    O5::Item: Clone,
    D0: Fn() -> O0::Item,
    D1: Fn() -> O1::Item,
    D2: Fn() -> O2::Item,
    D3: Fn() -> O3::Item,
    D4: Fn() -> O4::Item,
    D5: Fn() -> O5::Item,
{
    r0: R0,
    o0: O0,
    o1: O1,
    o2: O2,
    o3: O3,
    o4: O4,
    o5: O5,
    o0_default_fn: D0,
    o1_default_fn: D1,
    o2_default_fn: D2,
    o3_default_fn: D3,
    o4_default_fn: D4,
    o5_default_fn: D5,

    o0_latest_value: Option<O0::Item>,
    o1_latest_value: Option<O1::Item>,
    o2_latest_value: Option<O2::Item>,
    o3_latest_value: Option<O3::Item>,
    o4_latest_value: Option<O4::Item>,
    o5_latest_value: Option<O5::Item>,
}

impl<R0, O0, O1, O2, O3, O4, O5, D0, D1, D2, D3, D4, D5> Iterator
    for ClampedZip1x6<R0, O0, O1, O2, O3, O4, O5, D0, D1, D2, D3, D4, D5>
where
    R0: Iterator,
    O0: Iterator,
    O0::Item: Clone,
    O1: Iterator,
    O1::Item: Clone,
    O2: Iterator,
    O2::Item: Clone,
    O3: Iterator,
    O3::Item: Clone,
    O4: Iterator,
    O4::Item: Clone,
    O5: Iterator,
    O5::Item: Clone,
    D0: Fn() -> O0::Item,
    D1: Fn() -> O1::Item,
    D2: Fn() -> O2::Item,
    D3: Fn() -> O3::Item,
    D4: Fn() -> O4::Item,
    D5: Fn() -> O5::Item,
{
    type Item = (
        R0::Item,
        O0::Item,
        O1::Item,
        O2::Item,
        O3::Item,
        O4::Item,
        O5::Item,
    );

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let r0_next = self.r0.next()?;
        let o0_next = self.o0.next().or_else(|| self.o0_latest_value.take());
        let o1_next = self.o1.next().or_else(|| self.o1_latest_value.take());
        let o2_next = self.o2.next().or_else(|| self.o2_latest_value.take());
        let o3_next = self.o3.next().or_else(|| self.o3_latest_value.take());
        let o4_next = self.o4.next().or_else(|| self.o4_latest_value.take());
        let o5_next = self.o5.next().or_else(|| self.o5_latest_value.take());

        self.o0_latest_value.clone_from(&o0_next);
        self.o1_latest_value.clone_from(&o1_next);
        self.o2_latest_value.clone_from(&o2_next);
        self.o3_latest_value.clone_from(&o3_next);
        self.o4_latest_value.clone_from(&o4_next);
        self.o5_latest_value.clone_from(&o5_next);

        Some((
            r0_next,
            o0_next.unwrap_or_else(|| (self.o0_default_fn)()),
            o1_next.unwrap_or_else(|| (self.o1_default_fn)()),
            o2_next.unwrap_or_else(|| (self.o2_default_fn)()),
            o3_next.unwrap_or_else(|| (self.o3_default_fn)()),
            o4_next.unwrap_or_else(|| (self.o4_default_fn)()),
            o5_next.unwrap_or_else(|| (self.o5_default_fn)()),
        ))
    }
}

/// Returns a new [`ClampedZip1x7`] iterator.
///
/// The number of elements in a clamped zip iterator corresponds to the number of elements in the
/// shortest of its required iterators (`r0`).
///
/// Optional iterators (`o0`, `o1`, `o2`, `o3`, `o4`, `o5`, `o6`) will repeat their latest values if they happen to be too short
/// to be zipped with the shortest of the required iterators.
///
/// If an optional iterator is not only too short but actually empty, its associated default function
/// (`o0_default_fn`, `o1_default_fn`, `o2_default_fn`, `o3_default_fn`, `o4_default_fn`, `o5_default_fn`, `o6_default_fn`) will be executed and the resulting value repeated as necessary.
pub fn clamped_zip_1x7<R0, O0, O1, O2, O3, O4, O5, O6, D0, D1, D2, D3, D4, D5, D6>(
    r0: R0,
    o0: O0,
    o0_default_fn: D0,
    o1: O1,
    o1_default_fn: D1,
    o2: O2,
    o2_default_fn: D2,
    o3: O3,
    o3_default_fn: D3,
    o4: O4,
    o4_default_fn: D4,
    o5: O5,
    o5_default_fn: D5,
    o6: O6,
    o6_default_fn: D6,
) -> ClampedZip1x7<
    R0::IntoIter,
    O0::IntoIter,
    O1::IntoIter,
    O2::IntoIter,
    O3::IntoIter,
    O4::IntoIter,
    O5::IntoIter,
    O6::IntoIter,
    D0,
    D1,
    D2,
    D3,
    D4,
    D5,
    D6,
>
where
    R0: IntoIterator,
    O0: IntoIterator,
    O0::Item: Clone,
    O1: IntoIterator,
    O1::Item: Clone,
    O2: IntoIterator,
    O2::Item: Clone,
    O3: IntoIterator,
    O3::Item: Clone,
    O4: IntoIterator,
    O4::Item: Clone,
    O5: IntoIterator,
    O5::Item: Clone,
    O6: IntoIterator,
    O6::Item: Clone,
    D0: Fn() -> O0::Item,
    D1: Fn() -> O1::Item,
    D2: Fn() -> O2::Item,
    D3: Fn() -> O3::Item,
    D4: Fn() -> O4::Item,
    D5: Fn() -> O5::Item,
    D6: Fn() -> O6::Item,
{
    ClampedZip1x7 {
        r0: r0.into_iter(),
        o0: o0.into_iter(),
        o1: o1.into_iter(),
        o2: o2.into_iter(),
        o3: o3.into_iter(),
        o4: o4.into_iter(),
        o5: o5.into_iter(),
        o6: o6.into_iter(),
        o0_default_fn,
        o1_default_fn,
        o2_default_fn,
        o3_default_fn,
        o4_default_fn,
        o5_default_fn,
        o6_default_fn,
        o0_latest_value: None,
        o1_latest_value: None,
        o2_latest_value: None,
        o3_latest_value: None,
        o4_latest_value: None,
        o5_latest_value: None,
        o6_latest_value: None,
    }
}

/// Implements a clamped zip iterator combinator with 2 required iterators and 2 optional
/// iterators.
///
/// See [`clamped_zip_1x7`] for more information.
pub struct ClampedZip1x7<R0, O0, O1, O2, O3, O4, O5, O6, D0, D1, D2, D3, D4, D5, D6>
where
    R0: Iterator,
    O0: Iterator,
    O0::Item: Clone,
    O1: Iterator,
    O1::Item: Clone,
    O2: Iterator,
    O2::Item: Clone,
    O3: Iterator,
    O3::Item: Clone,
    O4: Iterator,
    O4::Item: Clone,
    O5: Iterator,
    O5::Item: Clone,
    O6: Iterator,
    O6::Item: Clone,
    D0: Fn() -> O0::Item,
    D1: Fn() -> O1::Item,
    D2: Fn() -> O2::Item,
    D3: Fn() -> O3::Item,
    D4: Fn() -> O4::Item,
    D5: Fn() -> O5::Item,
    D6: Fn() -> O6::Item,
{
    r0: R0,
    o0: O0,
    o1: O1,
    o2: O2,
    o3: O3,
    o4: O4,
    o5: O5,
    o6: O6,
    o0_default_fn: D0,
    o1_default_fn: D1,
    o2_default_fn: D2,
    o3_default_fn: D3,
    o4_default_fn: D4,
    o5_default_fn: D5,
    o6_default_fn: D6,

    o0_latest_value: Option<O0::Item>,
    o1_latest_value: Option<O1::Item>,
    o2_latest_value: Option<O2::Item>,
    o3_latest_value: Option<O3::Item>,
    o4_latest_value: Option<O4::Item>,
    o5_latest_value: Option<O5::Item>,
    o6_latest_value: Option<O6::Item>,
}

impl<R0, O0, O1, O2, O3, O4, O5, O6, D0, D1, D2, D3, D4, D5, D6> Iterator
    for ClampedZip1x7<R0, O0, O1, O2, O3, O4, O5, O6, D0, D1, D2, D3, D4, D5, D6>
where
    R0: Iterator,
    O0: Iterator,
    O0::Item: Clone,
    O1: Iterator,
    O1::Item: Clone,
    O2: Iterator,
    O2::Item: Clone,
    O3: Iterator,
    O3::Item: Clone,
    O4: Iterator,
    O4::Item: Clone,
    O5: Iterator,
    O5::Item: Clone,
    O6: Iterator,
    O6::Item: Clone,
    D0: Fn() -> O0::Item,
    D1: Fn() -> O1::Item,
    D2: Fn() -> O2::Item,
    D3: Fn() -> O3::Item,
    D4: Fn() -> O4::Item,
    D5: Fn() -> O5::Item,
    D6: Fn() -> O6::Item,
{
    type Item = (
        R0::Item,
        O0::Item,
        O1::Item,
        O2::Item,
        O3::Item,
        O4::Item,
        O5::Item,
        O6::Item,
    );

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let r0_next = self.r0.next()?;
        let o0_next = self.o0.next().or_else(|| self.o0_latest_value.take());
        let o1_next = self.o1.next().or_else(|| self.o1_latest_value.take());
        let o2_next = self.o2.next().or_else(|| self.o2_latest_value.take());
        let o3_next = self.o3.next().or_else(|| self.o3_latest_value.take());
        let o4_next = self.o4.next().or_else(|| self.o4_latest_value.take());
        let o5_next = self.o5.next().or_else(|| self.o5_latest_value.take());
        let o6_next = self.o6.next().or_else(|| self.o6_latest_value.take());

        self.o0_latest_value.clone_from(&o0_next);
        self.o1_latest_value.clone_from(&o1_next);
        self.o2_latest_value.clone_from(&o2_next);
        self.o3_latest_value.clone_from(&o3_next);
        self.o4_latest_value.clone_from(&o4_next);
        self.o5_latest_value.clone_from(&o5_next);
        self.o6_latest_value.clone_from(&o6_next);

        Some((
            r0_next,
            o0_next.unwrap_or_else(|| (self.o0_default_fn)()),
            o1_next.unwrap_or_else(|| (self.o1_default_fn)()),
            o2_next.unwrap_or_else(|| (self.o2_default_fn)()),
            o3_next.unwrap_or_else(|| (self.o3_default_fn)()),
            o4_next.unwrap_or_else(|| (self.o4_default_fn)()),
            o5_next.unwrap_or_else(|| (self.o5_default_fn)()),
            o6_next.unwrap_or_else(|| (self.o6_default_fn)()),
        ))
    }
}

/// Returns a new [`ClampedZip1x8`] iterator.
///
/// The number of elements in a clamped zip iterator corresponds to the number of elements in the
/// shortest of its required iterators (`r0`).
///
/// Optional iterators (`o0`, `o1`, `o2`, `o3`, `o4`, `o5`, `o6`, `o7`) will repeat their latest values if they happen to be too short
/// to be zipped with the shortest of the required iterators.
///
/// If an optional iterator is not only too short but actually empty, its associated default function
/// (`o0_default_fn`, `o1_default_fn`, `o2_default_fn`, `o3_default_fn`, `o4_default_fn`, `o5_default_fn`, `o6_default_fn`, `o7_default_fn`) will be executed and the resulting value repeated as necessary.
pub fn clamped_zip_1x8<R0, O0, O1, O2, O3, O4, O5, O6, O7, D0, D1, D2, D3, D4, D5, D6, D7>(
    r0: R0,
    o0: O0,
    o0_default_fn: D0,
    o1: O1,
    o1_default_fn: D1,
    o2: O2,
    o2_default_fn: D2,
    o3: O3,
    o3_default_fn: D3,
    o4: O4,
    o4_default_fn: D4,
    o5: O5,
    o5_default_fn: D5,
    o6: O6,
    o6_default_fn: D6,
    o7: O7,
    o7_default_fn: D7,
) -> ClampedZip1x8<
    R0::IntoIter,
    O0::IntoIter,
    O1::IntoIter,
    O2::IntoIter,
    O3::IntoIter,
    O4::IntoIter,
    O5::IntoIter,
    O6::IntoIter,
    O7::IntoIter,
    D0,
    D1,
    D2,
    D3,
    D4,
    D5,
    D6,
    D7,
>
where
    R0: IntoIterator,
    O0: IntoIterator,
    O0::Item: Clone,
    O1: IntoIterator,
    O1::Item: Clone,
    O2: IntoIterator,
    O2::Item: Clone,
    O3: IntoIterator,
    O3::Item: Clone,
    O4: IntoIterator,
    O4::Item: Clone,
    O5: IntoIterator,
    O5::Item: Clone,
    O6: IntoIterator,
    O6::Item: Clone,
    O7: IntoIterator,
    O7::Item: Clone,
    D0: Fn() -> O0::Item,
    D1: Fn() -> O1::Item,
    D2: Fn() -> O2::Item,
    D3: Fn() -> O3::Item,
    D4: Fn() -> O4::Item,
    D5: Fn() -> O5::Item,
    D6: Fn() -> O6::Item,
    D7: Fn() -> O7::Item,
{
    ClampedZip1x8 {
        r0: r0.into_iter(),
        o0: o0.into_iter(),
        o1: o1.into_iter(),
        o2: o2.into_iter(),
        o3: o3.into_iter(),
        o4: o4.into_iter(),
        o5: o5.into_iter(),
        o6: o6.into_iter(),
        o7: o7.into_iter(),
        o0_default_fn,
        o1_default_fn,
        o2_default_fn,
        o3_default_fn,
        o4_default_fn,
        o5_default_fn,
        o6_default_fn,
        o7_default_fn,
        o0_latest_value: None,
        o1_latest_value: None,
        o2_latest_value: None,
        o3_latest_value: None,
        o4_latest_value: None,
        o5_latest_value: None,
        o6_latest_value: None,
        o7_latest_value: None,
    }
}

/// Implements a clamped zip iterator combinator with 2 required iterators and 2 optional
/// iterators.
///
/// See [`clamped_zip_1x8`] for more information.
pub struct ClampedZip1x8<R0, O0, O1, O2, O3, O4, O5, O6, O7, D0, D1, D2, D3, D4, D5, D6, D7>
where
    R0: Iterator,
    O0: Iterator,
    O0::Item: Clone,
    O1: Iterator,
    O1::Item: Clone,
    O2: Iterator,
    O2::Item: Clone,
    O3: Iterator,
    O3::Item: Clone,
    O4: Iterator,
    O4::Item: Clone,
    O5: Iterator,
    O5::Item: Clone,
    O6: Iterator,
    O6::Item: Clone,
    O7: Iterator,
    O7::Item: Clone,
    D0: Fn() -> O0::Item,
    D1: Fn() -> O1::Item,
    D2: Fn() -> O2::Item,
    D3: Fn() -> O3::Item,
    D4: Fn() -> O4::Item,
    D5: Fn() -> O5::Item,
    D6: Fn() -> O6::Item,
    D7: Fn() -> O7::Item,
{
    r0: R0,
    o0: O0,
    o1: O1,
    o2: O2,
    o3: O3,
    o4: O4,
    o5: O5,
    o6: O6,
    o7: O7,
    o0_default_fn: D0,
    o1_default_fn: D1,
    o2_default_fn: D2,
    o3_default_fn: D3,
    o4_default_fn: D4,
    o5_default_fn: D5,
    o6_default_fn: D6,
    o7_default_fn: D7,

    o0_latest_value: Option<O0::Item>,
    o1_latest_value: Option<O1::Item>,
    o2_latest_value: Option<O2::Item>,
    o3_latest_value: Option<O3::Item>,
    o4_latest_value: Option<O4::Item>,
    o5_latest_value: Option<O5::Item>,
    o6_latest_value: Option<O6::Item>,
    o7_latest_value: Option<O7::Item>,
}

impl<R0, O0, O1, O2, O3, O4, O5, O6, O7, D0, D1, D2, D3, D4, D5, D6, D7> Iterator
    for ClampedZip1x8<R0, O0, O1, O2, O3, O4, O5, O6, O7, D0, D1, D2, D3, D4, D5, D6, D7>
where
    R0: Iterator,
    O0: Iterator,
    O0::Item: Clone,
    O1: Iterator,
    O1::Item: Clone,
    O2: Iterator,
    O2::Item: Clone,
    O3: Iterator,
    O3::Item: Clone,
    O4: Iterator,
    O4::Item: Clone,
    O5: Iterator,
    O5::Item: Clone,
    O6: Iterator,
    O6::Item: Clone,
    O7: Iterator,
    O7::Item: Clone,
    D0: Fn() -> O0::Item,
    D1: Fn() -> O1::Item,
    D2: Fn() -> O2::Item,
    D3: Fn() -> O3::Item,
    D4: Fn() -> O4::Item,
    D5: Fn() -> O5::Item,
    D6: Fn() -> O6::Item,
    D7: Fn() -> O7::Item,
{
    type Item = (
        R0::Item,
        O0::Item,
        O1::Item,
        O2::Item,
        O3::Item,
        O4::Item,
        O5::Item,
        O6::Item,
        O7::Item,
    );

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let r0_next = self.r0.next()?;
        let o0_next = self.o0.next().or_else(|| self.o0_latest_value.take());
        let o1_next = self.o1.next().or_else(|| self.o1_latest_value.take());
        let o2_next = self.o2.next().or_else(|| self.o2_latest_value.take());
        let o3_next = self.o3.next().or_else(|| self.o3_latest_value.take());
        let o4_next = self.o4.next().or_else(|| self.o4_latest_value.take());
        let o5_next = self.o5.next().or_else(|| self.o5_latest_value.take());
        let o6_next = self.o6.next().or_else(|| self.o6_latest_value.take());
        let o7_next = self.o7.next().or_else(|| self.o7_latest_value.take());

        self.o0_latest_value.clone_from(&o0_next);
        self.o1_latest_value.clone_from(&o1_next);
        self.o2_latest_value.clone_from(&o2_next);
        self.o3_latest_value.clone_from(&o3_next);
        self.o4_latest_value.clone_from(&o4_next);
        self.o5_latest_value.clone_from(&o5_next);
        self.o6_latest_value.clone_from(&o6_next);
        self.o7_latest_value.clone_from(&o7_next);

        Some((
            r0_next,
            o0_next.unwrap_or_else(|| (self.o0_default_fn)()),
            o1_next.unwrap_or_else(|| (self.o1_default_fn)()),
            o2_next.unwrap_or_else(|| (self.o2_default_fn)()),
            o3_next.unwrap_or_else(|| (self.o3_default_fn)()),
            o4_next.unwrap_or_else(|| (self.o4_default_fn)()),
            o5_next.unwrap_or_else(|| (self.o5_default_fn)()),
            o6_next.unwrap_or_else(|| (self.o6_default_fn)()),
            o7_next.unwrap_or_else(|| (self.o7_default_fn)()),
        ))
    }
}

/// Returns a new [`ClampedZip1x9`] iterator.
///
/// The number of elements in a clamped zip iterator corresponds to the number of elements in the
/// shortest of its required iterators (`r0`).
///
/// Optional iterators (`o0`, `o1`, `o2`, `o3`, `o4`, `o5`, `o6`, `o7`, `o8`) will repeat their latest values if they happen to be too short
/// to be zipped with the shortest of the required iterators.
///
/// If an optional iterator is not only too short but actually empty, its associated default function
/// (`o0_default_fn`, `o1_default_fn`, `o2_default_fn`, `o3_default_fn`, `o4_default_fn`, `o5_default_fn`, `o6_default_fn`, `o7_default_fn`, `o8_default_fn`) will be executed and the resulting value repeated as necessary.
pub fn clamped_zip_1x9<R0, O0, O1, O2, O3, O4, O5, O6, O7, O8, D0, D1, D2, D3, D4, D5, D6, D7, D8>(
    r0: R0,
    o0: O0,
    o0_default_fn: D0,
    o1: O1,
    o1_default_fn: D1,
    o2: O2,
    o2_default_fn: D2,
    o3: O3,
    o3_default_fn: D3,
    o4: O4,
    o4_default_fn: D4,
    o5: O5,
    o5_default_fn: D5,
    o6: O6,
    o6_default_fn: D6,
    o7: O7,
    o7_default_fn: D7,
    o8: O8,
    o8_default_fn: D8,
) -> ClampedZip1x9<
    R0::IntoIter,
    O0::IntoIter,
    O1::IntoIter,
    O2::IntoIter,
    O3::IntoIter,
    O4::IntoIter,
    O5::IntoIter,
    O6::IntoIter,
    O7::IntoIter,
    O8::IntoIter,
    D0,
    D1,
    D2,
    D3,
    D4,
    D5,
    D6,
    D7,
    D8,
>
where
    R0: IntoIterator,
    O0: IntoIterator,
    O0::Item: Clone,
    O1: IntoIterator,
    O1::Item: Clone,
    O2: IntoIterator,
    O2::Item: Clone,
    O3: IntoIterator,
    O3::Item: Clone,
    O4: IntoIterator,
    O4::Item: Clone,
    O5: IntoIterator,
    O5::Item: Clone,
    O6: IntoIterator,
    O6::Item: Clone,
    O7: IntoIterator,
    O7::Item: Clone,
    O8: IntoIterator,
    O8::Item: Clone,
    D0: Fn() -> O0::Item,
    D1: Fn() -> O1::Item,
    D2: Fn() -> O2::Item,
    D3: Fn() -> O3::Item,
    D4: Fn() -> O4::Item,
    D5: Fn() -> O5::Item,
    D6: Fn() -> O6::Item,
    D7: Fn() -> O7::Item,
    D8: Fn() -> O8::Item,
{
    ClampedZip1x9 {
        r0: r0.into_iter(),
        o0: o0.into_iter(),
        o1: o1.into_iter(),
        o2: o2.into_iter(),
        o3: o3.into_iter(),
        o4: o4.into_iter(),
        o5: o5.into_iter(),
        o6: o6.into_iter(),
        o7: o7.into_iter(),
        o8: o8.into_iter(),
        o0_default_fn,
        o1_default_fn,
        o2_default_fn,
        o3_default_fn,
        o4_default_fn,
        o5_default_fn,
        o6_default_fn,
        o7_default_fn,
        o8_default_fn,
        o0_latest_value: None,
        o1_latest_value: None,
        o2_latest_value: None,
        o3_latest_value: None,
        o4_latest_value: None,
        o5_latest_value: None,
        o6_latest_value: None,
        o7_latest_value: None,
        o8_latest_value: None,
    }
}

/// Implements a clamped zip iterator combinator with 2 required iterators and 2 optional
/// iterators.
///
/// See [`clamped_zip_1x9`] for more information.
pub struct ClampedZip1x9<R0, O0, O1, O2, O3, O4, O5, O6, O7, O8, D0, D1, D2, D3, D4, D5, D6, D7, D8>
where
    R0: Iterator,
    O0: Iterator,
    O0::Item: Clone,
    O1: Iterator,
    O1::Item: Clone,
    O2: Iterator,
    O2::Item: Clone,
    O3: Iterator,
    O3::Item: Clone,
    O4: Iterator,
    O4::Item: Clone,
    O5: Iterator,
    O5::Item: Clone,
    O6: Iterator,
    O6::Item: Clone,
    O7: Iterator,
    O7::Item: Clone,
    O8: Iterator,
    O8::Item: Clone,
    D0: Fn() -> O0::Item,
    D1: Fn() -> O1::Item,
    D2: Fn() -> O2::Item,
    D3: Fn() -> O3::Item,
    D4: Fn() -> O4::Item,
    D5: Fn() -> O5::Item,
    D6: Fn() -> O6::Item,
    D7: Fn() -> O7::Item,
    D8: Fn() -> O8::Item,
{
    r0: R0,
    o0: O0,
    o1: O1,
    o2: O2,
    o3: O3,
    o4: O4,
    o5: O5,
    o6: O6,
    o7: O7,
    o8: O8,
    o0_default_fn: D0,
    o1_default_fn: D1,
    o2_default_fn: D2,
    o3_default_fn: D3,
    o4_default_fn: D4,
    o5_default_fn: D5,
    o6_default_fn: D6,
    o7_default_fn: D7,
    o8_default_fn: D8,

    o0_latest_value: Option<O0::Item>,
    o1_latest_value: Option<O1::Item>,
    o2_latest_value: Option<O2::Item>,
    o3_latest_value: Option<O3::Item>,
    o4_latest_value: Option<O4::Item>,
    o5_latest_value: Option<O5::Item>,
    o6_latest_value: Option<O6::Item>,
    o7_latest_value: Option<O7::Item>,
    o8_latest_value: Option<O8::Item>,
}

impl<R0, O0, O1, O2, O3, O4, O5, O6, O7, O8, D0, D1, D2, D3, D4, D5, D6, D7, D8> Iterator
    for ClampedZip1x9<R0, O0, O1, O2, O3, O4, O5, O6, O7, O8, D0, D1, D2, D3, D4, D5, D6, D7, D8>
where
    R0: Iterator,
    O0: Iterator,
    O0::Item: Clone,
    O1: Iterator,
    O1::Item: Clone,
    O2: Iterator,
    O2::Item: Clone,
    O3: Iterator,
    O3::Item: Clone,
    O4: Iterator,
    O4::Item: Clone,
    O5: Iterator,
    O5::Item: Clone,
    O6: Iterator,
    O6::Item: Clone,
    O7: Iterator,
    O7::Item: Clone,
    O8: Iterator,
    O8::Item: Clone,
    D0: Fn() -> O0::Item,
    D1: Fn() -> O1::Item,
    D2: Fn() -> O2::Item,
    D3: Fn() -> O3::Item,
    D4: Fn() -> O4::Item,
    D5: Fn() -> O5::Item,
    D6: Fn() -> O6::Item,
    D7: Fn() -> O7::Item,
    D8: Fn() -> O8::Item,
{
    type Item = (
        R0::Item,
        O0::Item,
        O1::Item,
        O2::Item,
        O3::Item,
        O4::Item,
        O5::Item,
        O6::Item,
        O7::Item,
        O8::Item,
    );

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let r0_next = self.r0.next()?;
        let o0_next = self.o0.next().or_else(|| self.o0_latest_value.take());
        let o1_next = self.o1.next().or_else(|| self.o1_latest_value.take());
        let o2_next = self.o2.next().or_else(|| self.o2_latest_value.take());
        let o3_next = self.o3.next().or_else(|| self.o3_latest_value.take());
        let o4_next = self.o4.next().or_else(|| self.o4_latest_value.take());
        let o5_next = self.o5.next().or_else(|| self.o5_latest_value.take());
        let o6_next = self.o6.next().or_else(|| self.o6_latest_value.take());
        let o7_next = self.o7.next().or_else(|| self.o7_latest_value.take());
        let o8_next = self.o8.next().or_else(|| self.o8_latest_value.take());

        self.o0_latest_value.clone_from(&o0_next);
        self.o1_latest_value.clone_from(&o1_next);
        self.o2_latest_value.clone_from(&o2_next);
        self.o3_latest_value.clone_from(&o3_next);
        self.o4_latest_value.clone_from(&o4_next);
        self.o5_latest_value.clone_from(&o5_next);
        self.o6_latest_value.clone_from(&o6_next);
        self.o7_latest_value.clone_from(&o7_next);
        self.o8_latest_value.clone_from(&o8_next);

        Some((
            r0_next,
            o0_next.unwrap_or_else(|| (self.o0_default_fn)()),
            o1_next.unwrap_or_else(|| (self.o1_default_fn)()),
            o2_next.unwrap_or_else(|| (self.o2_default_fn)()),
            o3_next.unwrap_or_else(|| (self.o3_default_fn)()),
            o4_next.unwrap_or_else(|| (self.o4_default_fn)()),
            o5_next.unwrap_or_else(|| (self.o5_default_fn)()),
            o6_next.unwrap_or_else(|| (self.o6_default_fn)()),
            o7_next.unwrap_or_else(|| (self.o7_default_fn)()),
            o8_next.unwrap_or_else(|| (self.o8_default_fn)()),
        ))
    }
}

/// Returns a new [`ClampedZip2x1`] iterator.
///
/// The number of elements in a clamped zip iterator corresponds to the number of elements in the
/// shortest of its required iterators (`r0`, `r1`).
///
/// Optional iterators (`o0`) will repeat their latest values if they happen to be too short
/// to be zipped with the shortest of the required iterators.
///
/// If an optional iterator is not only too short but actually empty, its associated default function
/// (`o0_default_fn`) will be executed and the resulting value repeated as necessary.
pub fn clamped_zip_2x1<R0, R1, O0, D0>(
    r0: R0,
    r1: R1,
    o0: O0,
    o0_default_fn: D0,
) -> ClampedZip2x1<R0::IntoIter, R1::IntoIter, O0::IntoIter, D0>
where
    R0: IntoIterator,
    R1: IntoIterator,
    O0: IntoIterator,
    O0::Item: Clone,
    D0: Fn() -> O0::Item,
{
    ClampedZip2x1 {
        r0: r0.into_iter(),
        r1: r1.into_iter(),
        o0: o0.into_iter(),
        o0_default_fn,
        o0_latest_value: None,
    }
}

/// Implements a clamped zip iterator combinator with 2 required iterators and 2 optional
/// iterators.
///
/// See [`clamped_zip_2x1`] for more information.
pub struct ClampedZip2x1<R0, R1, O0, D0>
where
    R0: Iterator,
    R1: Iterator,
    O0: Iterator,
    O0::Item: Clone,
    D0: Fn() -> O0::Item,
{
    r0: R0,
    r1: R1,
    o0: O0,
    o0_default_fn: D0,

    o0_latest_value: Option<O0::Item>,
}

impl<R0, R1, O0, D0> Iterator for ClampedZip2x1<R0, R1, O0, D0>
where
    R0: Iterator,
    R1: Iterator,
    O0: Iterator,
    O0::Item: Clone,
    D0: Fn() -> O0::Item,
{
    type Item = (R0::Item, R1::Item, O0::Item);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let r0_next = self.r0.next()?;
        let r1_next = self.r1.next()?;
        let o0_next = self.o0.next().or_else(|| self.o0_latest_value.take());

        self.o0_latest_value.clone_from(&o0_next);

        Some((
            r0_next,
            r1_next,
            o0_next.unwrap_or_else(|| (self.o0_default_fn)()),
        ))
    }
}

/// Returns a new [`ClampedZip2x2`] iterator.
///
/// The number of elements in a clamped zip iterator corresponds to the number of elements in the
/// shortest of its required iterators (`r0`, `r1`).
///
/// Optional iterators (`o0`, `o1`) will repeat their latest values if they happen to be too short
/// to be zipped with the shortest of the required iterators.
///
/// If an optional iterator is not only too short but actually empty, its associated default function
/// (`o0_default_fn`, `o1_default_fn`) will be executed and the resulting value repeated as necessary.
pub fn clamped_zip_2x2<R0, R1, O0, O1, D0, D1>(
    r0: R0,
    r1: R1,
    o0: O0,
    o0_default_fn: D0,
    o1: O1,
    o1_default_fn: D1,
) -> ClampedZip2x2<R0::IntoIter, R1::IntoIter, O0::IntoIter, O1::IntoIter, D0, D1>
where
    R0: IntoIterator,
    R1: IntoIterator,
    O0: IntoIterator,
    O0::Item: Clone,
    O1: IntoIterator,
    O1::Item: Clone,
    D0: Fn() -> O0::Item,
    D1: Fn() -> O1::Item,
{
    ClampedZip2x2 {
        r0: r0.into_iter(),
        r1: r1.into_iter(),
        o0: o0.into_iter(),
        o1: o1.into_iter(),
        o0_default_fn,
        o1_default_fn,
        o0_latest_value: None,
        o1_latest_value: None,
    }
}

/// Implements a clamped zip iterator combinator with 2 required iterators and 2 optional
/// iterators.
///
/// See [`clamped_zip_2x2`] for more information.
pub struct ClampedZip2x2<R0, R1, O0, O1, D0, D1>
where
    R0: Iterator,
    R1: Iterator,
    O0: Iterator,
    O0::Item: Clone,
    O1: Iterator,
    O1::Item: Clone,
    D0: Fn() -> O0::Item,
    D1: Fn() -> O1::Item,
{
    r0: R0,
    r1: R1,
    o0: O0,
    o1: O1,
    o0_default_fn: D0,
    o1_default_fn: D1,

    o0_latest_value: Option<O0::Item>,
    o1_latest_value: Option<O1::Item>,
}

impl<R0, R1, O0, O1, D0, D1> Iterator for ClampedZip2x2<R0, R1, O0, O1, D0, D1>
where
    R0: Iterator,
    R1: Iterator,
    O0: Iterator,
    O0::Item: Clone,
    O1: Iterator,
    O1::Item: Clone,
    D0: Fn() -> O0::Item,
    D1: Fn() -> O1::Item,
{
    type Item = (R0::Item, R1::Item, O0::Item, O1::Item);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let r0_next = self.r0.next()?;
        let r1_next = self.r1.next()?;
        let o0_next = self.o0.next().or_else(|| self.o0_latest_value.take());
        let o1_next = self.o1.next().or_else(|| self.o1_latest_value.take());

        self.o0_latest_value.clone_from(&o0_next);
        self.o1_latest_value.clone_from(&o1_next);

        Some((
            r0_next,
            r1_next,
            o0_next.unwrap_or_else(|| (self.o0_default_fn)()),
            o1_next.unwrap_or_else(|| (self.o1_default_fn)()),
        ))
    }
}

/// Returns a new [`ClampedZip2x3`] iterator.
///
/// The number of elements in a clamped zip iterator corresponds to the number of elements in the
/// shortest of its required iterators (`r0`, `r1`).
///
/// Optional iterators (`o0`, `o1`, `o2`) will repeat their latest values if they happen to be too short
/// to be zipped with the shortest of the required iterators.
///
/// If an optional iterator is not only too short but actually empty, its associated default function
/// (`o0_default_fn`, `o1_default_fn`, `o2_default_fn`) will be executed and the resulting value repeated as necessary.
pub fn clamped_zip_2x3<R0, R1, O0, O1, O2, D0, D1, D2>(
    r0: R0,
    r1: R1,
    o0: O0,
    o0_default_fn: D0,
    o1: O1,
    o1_default_fn: D1,
    o2: O2,
    o2_default_fn: D2,
) -> ClampedZip2x3<R0::IntoIter, R1::IntoIter, O0::IntoIter, O1::IntoIter, O2::IntoIter, D0, D1, D2>
where
    R0: IntoIterator,
    R1: IntoIterator,
    O0: IntoIterator,
    O0::Item: Clone,
    O1: IntoIterator,
    O1::Item: Clone,
    O2: IntoIterator,
    O2::Item: Clone,
    D0: Fn() -> O0::Item,
    D1: Fn() -> O1::Item,
    D2: Fn() -> O2::Item,
{
    ClampedZip2x3 {
        r0: r0.into_iter(),
        r1: r1.into_iter(),
        o0: o0.into_iter(),
        o1: o1.into_iter(),
        o2: o2.into_iter(),
        o0_default_fn,
        o1_default_fn,
        o2_default_fn,
        o0_latest_value: None,
        o1_latest_value: None,
        o2_latest_value: None,
    }
}

/// Implements a clamped zip iterator combinator with 2 required iterators and 2 optional
/// iterators.
///
/// See [`clamped_zip_2x3`] for more information.
pub struct ClampedZip2x3<R0, R1, O0, O1, O2, D0, D1, D2>
where
    R0: Iterator,
    R1: Iterator,
    O0: Iterator,
    O0::Item: Clone,
    O1: Iterator,
    O1::Item: Clone,
    O2: Iterator,
    O2::Item: Clone,
    D0: Fn() -> O0::Item,
    D1: Fn() -> O1::Item,
    D2: Fn() -> O2::Item,
{
    r0: R0,
    r1: R1,
    o0: O0,
    o1: O1,
    o2: O2,
    o0_default_fn: D0,
    o1_default_fn: D1,
    o2_default_fn: D2,

    o0_latest_value: Option<O0::Item>,
    o1_latest_value: Option<O1::Item>,
    o2_latest_value: Option<O2::Item>,
}

impl<R0, R1, O0, O1, O2, D0, D1, D2> Iterator for ClampedZip2x3<R0, R1, O0, O1, O2, D0, D1, D2>
where
    R0: Iterator,
    R1: Iterator,
    O0: Iterator,
    O0::Item: Clone,
    O1: Iterator,
    O1::Item: Clone,
    O2: Iterator,
    O2::Item: Clone,
    D0: Fn() -> O0::Item,
    D1: Fn() -> O1::Item,
    D2: Fn() -> O2::Item,
{
    type Item = (R0::Item, R1::Item, O0::Item, O1::Item, O2::Item);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let r0_next = self.r0.next()?;
        let r1_next = self.r1.next()?;
        let o0_next = self.o0.next().or_else(|| self.o0_latest_value.take());
        let o1_next = self.o1.next().or_else(|| self.o1_latest_value.take());
        let o2_next = self.o2.next().or_else(|| self.o2_latest_value.take());

        self.o0_latest_value.clone_from(&o0_next);
        self.o1_latest_value.clone_from(&o1_next);
        self.o2_latest_value.clone_from(&o2_next);

        Some((
            r0_next,
            r1_next,
            o0_next.unwrap_or_else(|| (self.o0_default_fn)()),
            o1_next.unwrap_or_else(|| (self.o1_default_fn)()),
            o2_next.unwrap_or_else(|| (self.o2_default_fn)()),
        ))
    }
}

/// Returns a new [`ClampedZip2x4`] iterator.
///
/// The number of elements in a clamped zip iterator corresponds to the number of elements in the
/// shortest of its required iterators (`r0`, `r1`).
///
/// Optional iterators (`o0`, `o1`, `o2`, `o3`) will repeat their latest values if they happen to be too short
/// to be zipped with the shortest of the required iterators.
///
/// If an optional iterator is not only too short but actually empty, its associated default function
/// (`o0_default_fn`, `o1_default_fn`, `o2_default_fn`, `o3_default_fn`) will be executed and the resulting value repeated as necessary.
pub fn clamped_zip_2x4<R0, R1, O0, O1, O2, O3, D0, D1, D2, D3>(
    r0: R0,
    r1: R1,
    o0: O0,
    o0_default_fn: D0,
    o1: O1,
    o1_default_fn: D1,
    o2: O2,
    o2_default_fn: D2,
    o3: O3,
    o3_default_fn: D3,
) -> ClampedZip2x4<
    R0::IntoIter,
    R1::IntoIter,
    O0::IntoIter,
    O1::IntoIter,
    O2::IntoIter,
    O3::IntoIter,
    D0,
    D1,
    D2,
    D3,
>
where
    R0: IntoIterator,
    R1: IntoIterator,
    O0: IntoIterator,
    O0::Item: Clone,
    O1: IntoIterator,
    O1::Item: Clone,
    O2: IntoIterator,
    O2::Item: Clone,
    O3: IntoIterator,
    O3::Item: Clone,
    D0: Fn() -> O0::Item,
    D1: Fn() -> O1::Item,
    D2: Fn() -> O2::Item,
    D3: Fn() -> O3::Item,
{
    ClampedZip2x4 {
        r0: r0.into_iter(),
        r1: r1.into_iter(),
        o0: o0.into_iter(),
        o1: o1.into_iter(),
        o2: o2.into_iter(),
        o3: o3.into_iter(),
        o0_default_fn,
        o1_default_fn,
        o2_default_fn,
        o3_default_fn,
        o0_latest_value: None,
        o1_latest_value: None,
        o2_latest_value: None,
        o3_latest_value: None,
    }
}

/// Implements a clamped zip iterator combinator with 2 required iterators and 2 optional
/// iterators.
///
/// See [`clamped_zip_2x4`] for more information.
pub struct ClampedZip2x4<R0, R1, O0, O1, O2, O3, D0, D1, D2, D3>
where
    R0: Iterator,
    R1: Iterator,
    O0: Iterator,
    O0::Item: Clone,
    O1: Iterator,
    O1::Item: Clone,
    O2: Iterator,
    O2::Item: Clone,
    O3: Iterator,
    O3::Item: Clone,
    D0: Fn() -> O0::Item,
    D1: Fn() -> O1::Item,
    D2: Fn() -> O2::Item,
    D3: Fn() -> O3::Item,
{
    r0: R0,
    r1: R1,
    o0: O0,
    o1: O1,
    o2: O2,
    o3: O3,
    o0_default_fn: D0,
    o1_default_fn: D1,
    o2_default_fn: D2,
    o3_default_fn: D3,

    o0_latest_value: Option<O0::Item>,
    o1_latest_value: Option<O1::Item>,
    o2_latest_value: Option<O2::Item>,
    o3_latest_value: Option<O3::Item>,
}

impl<R0, R1, O0, O1, O2, O3, D0, D1, D2, D3> Iterator
    for ClampedZip2x4<R0, R1, O0, O1, O2, O3, D0, D1, D2, D3>
where
    R0: Iterator,
    R1: Iterator,
    O0: Iterator,
    O0::Item: Clone,
    O1: Iterator,
    O1::Item: Clone,
    O2: Iterator,
    O2::Item: Clone,
    O3: Iterator,
    O3::Item: Clone,
    D0: Fn() -> O0::Item,
    D1: Fn() -> O1::Item,
    D2: Fn() -> O2::Item,
    D3: Fn() -> O3::Item,
{
    type Item = (R0::Item, R1::Item, O0::Item, O1::Item, O2::Item, O3::Item);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let r0_next = self.r0.next()?;
        let r1_next = self.r1.next()?;
        let o0_next = self.o0.next().or_else(|| self.o0_latest_value.take());
        let o1_next = self.o1.next().or_else(|| self.o1_latest_value.take());
        let o2_next = self.o2.next().or_else(|| self.o2_latest_value.take());
        let o3_next = self.o3.next().or_else(|| self.o3_latest_value.take());

        self.o0_latest_value.clone_from(&o0_next);
        self.o1_latest_value.clone_from(&o1_next);
        self.o2_latest_value.clone_from(&o2_next);
        self.o3_latest_value.clone_from(&o3_next);

        Some((
            r0_next,
            r1_next,
            o0_next.unwrap_or_else(|| (self.o0_default_fn)()),
            o1_next.unwrap_or_else(|| (self.o1_default_fn)()),
            o2_next.unwrap_or_else(|| (self.o2_default_fn)()),
            o3_next.unwrap_or_else(|| (self.o3_default_fn)()),
        ))
    }
}

/// Returns a new [`ClampedZip2x5`] iterator.
///
/// The number of elements in a clamped zip iterator corresponds to the number of elements in the
/// shortest of its required iterators (`r0`, `r1`).
///
/// Optional iterators (`o0`, `o1`, `o2`, `o3`, `o4`) will repeat their latest values if they happen to be too short
/// to be zipped with the shortest of the required iterators.
///
/// If an optional iterator is not only too short but actually empty, its associated default function
/// (`o0_default_fn`, `o1_default_fn`, `o2_default_fn`, `o3_default_fn`, `o4_default_fn`) will be executed and the resulting value repeated as necessary.
pub fn clamped_zip_2x5<R0, R1, O0, O1, O2, O3, O4, D0, D1, D2, D3, D4>(
    r0: R0,
    r1: R1,
    o0: O0,
    o0_default_fn: D0,
    o1: O1,
    o1_default_fn: D1,
    o2: O2,
    o2_default_fn: D2,
    o3: O3,
    o3_default_fn: D3,
    o4: O4,
    o4_default_fn: D4,
) -> ClampedZip2x5<
    R0::IntoIter,
    R1::IntoIter,
    O0::IntoIter,
    O1::IntoIter,
    O2::IntoIter,
    O3::IntoIter,
    O4::IntoIter,
    D0,
    D1,
    D2,
    D3,
    D4,
>
where
    R0: IntoIterator,
    R1: IntoIterator,
    O0: IntoIterator,
    O0::Item: Clone,
    O1: IntoIterator,
    O1::Item: Clone,
    O2: IntoIterator,
    O2::Item: Clone,
    O3: IntoIterator,
    O3::Item: Clone,
    O4: IntoIterator,
    O4::Item: Clone,
    D0: Fn() -> O0::Item,
    D1: Fn() -> O1::Item,
    D2: Fn() -> O2::Item,
    D3: Fn() -> O3::Item,
    D4: Fn() -> O4::Item,
{
    ClampedZip2x5 {
        r0: r0.into_iter(),
        r1: r1.into_iter(),
        o0: o0.into_iter(),
        o1: o1.into_iter(),
        o2: o2.into_iter(),
        o3: o3.into_iter(),
        o4: o4.into_iter(),
        o0_default_fn,
        o1_default_fn,
        o2_default_fn,
        o3_default_fn,
        o4_default_fn,
        o0_latest_value: None,
        o1_latest_value: None,
        o2_latest_value: None,
        o3_latest_value: None,
        o4_latest_value: None,
    }
}

/// Implements a clamped zip iterator combinator with 2 required iterators and 2 optional
/// iterators.
///
/// See [`clamped_zip_2x5`] for more information.
pub struct ClampedZip2x5<R0, R1, O0, O1, O2, O3, O4, D0, D1, D2, D3, D4>
where
    R0: Iterator,
    R1: Iterator,
    O0: Iterator,
    O0::Item: Clone,
    O1: Iterator,
    O1::Item: Clone,
    O2: Iterator,
    O2::Item: Clone,
    O3: Iterator,
    O3::Item: Clone,
    O4: Iterator,
    O4::Item: Clone,
    D0: Fn() -> O0::Item,
    D1: Fn() -> O1::Item,
    D2: Fn() -> O2::Item,
    D3: Fn() -> O3::Item,
    D4: Fn() -> O4::Item,
{
    r0: R0,
    r1: R1,
    o0: O0,
    o1: O1,
    o2: O2,
    o3: O3,
    o4: O4,
    o0_default_fn: D0,
    o1_default_fn: D1,
    o2_default_fn: D2,
    o3_default_fn: D3,
    o4_default_fn: D4,

    o0_latest_value: Option<O0::Item>,
    o1_latest_value: Option<O1::Item>,
    o2_latest_value: Option<O2::Item>,
    o3_latest_value: Option<O3::Item>,
    o4_latest_value: Option<O4::Item>,
}

impl<R0, R1, O0, O1, O2, O3, O4, D0, D1, D2, D3, D4> Iterator
    for ClampedZip2x5<R0, R1, O0, O1, O2, O3, O4, D0, D1, D2, D3, D4>
where
    R0: Iterator,
    R1: Iterator,
    O0: Iterator,
    O0::Item: Clone,
    O1: Iterator,
    O1::Item: Clone,
    O2: Iterator,
    O2::Item: Clone,
    O3: Iterator,
    O3::Item: Clone,
    O4: Iterator,
    O4::Item: Clone,
    D0: Fn() -> O0::Item,
    D1: Fn() -> O1::Item,
    D2: Fn() -> O2::Item,
    D3: Fn() -> O3::Item,
    D4: Fn() -> O4::Item,
{
    type Item = (
        R0::Item,
        R1::Item,
        O0::Item,
        O1::Item,
        O2::Item,
        O3::Item,
        O4::Item,
    );

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let r0_next = self.r0.next()?;
        let r1_next = self.r1.next()?;
        let o0_next = self.o0.next().or_else(|| self.o0_latest_value.take());
        let o1_next = self.o1.next().or_else(|| self.o1_latest_value.take());
        let o2_next = self.o2.next().or_else(|| self.o2_latest_value.take());
        let o3_next = self.o3.next().or_else(|| self.o3_latest_value.take());
        let o4_next = self.o4.next().or_else(|| self.o4_latest_value.take());

        self.o0_latest_value.clone_from(&o0_next);
        self.o1_latest_value.clone_from(&o1_next);
        self.o2_latest_value.clone_from(&o2_next);
        self.o3_latest_value.clone_from(&o3_next);
        self.o4_latest_value.clone_from(&o4_next);

        Some((
            r0_next,
            r1_next,
            o0_next.unwrap_or_else(|| (self.o0_default_fn)()),
            o1_next.unwrap_or_else(|| (self.o1_default_fn)()),
            o2_next.unwrap_or_else(|| (self.o2_default_fn)()),
            o3_next.unwrap_or_else(|| (self.o3_default_fn)()),
            o4_next.unwrap_or_else(|| (self.o4_default_fn)()),
        ))
    }
}

/// Returns a new [`ClampedZip2x6`] iterator.
///
/// The number of elements in a clamped zip iterator corresponds to the number of elements in the
/// shortest of its required iterators (`r0`, `r1`).
///
/// Optional iterators (`o0`, `o1`, `o2`, `o3`, `o4`, `o5`) will repeat their latest values if they happen to be too short
/// to be zipped with the shortest of the required iterators.
///
/// If an optional iterator is not only too short but actually empty, its associated default function
/// (`o0_default_fn`, `o1_default_fn`, `o2_default_fn`, `o3_default_fn`, `o4_default_fn`, `o5_default_fn`) will be executed and the resulting value repeated as necessary.
pub fn clamped_zip_2x6<R0, R1, O0, O1, O2, O3, O4, O5, D0, D1, D2, D3, D4, D5>(
    r0: R0,
    r1: R1,
    o0: O0,
    o0_default_fn: D0,
    o1: O1,
    o1_default_fn: D1,
    o2: O2,
    o2_default_fn: D2,
    o3: O3,
    o3_default_fn: D3,
    o4: O4,
    o4_default_fn: D4,
    o5: O5,
    o5_default_fn: D5,
) -> ClampedZip2x6<
    R0::IntoIter,
    R1::IntoIter,
    O0::IntoIter,
    O1::IntoIter,
    O2::IntoIter,
    O3::IntoIter,
    O4::IntoIter,
    O5::IntoIter,
    D0,
    D1,
    D2,
    D3,
    D4,
    D5,
>
where
    R0: IntoIterator,
    R1: IntoIterator,
    O0: IntoIterator,
    O0::Item: Clone,
    O1: IntoIterator,
    O1::Item: Clone,
    O2: IntoIterator,
    O2::Item: Clone,
    O3: IntoIterator,
    O3::Item: Clone,
    O4: IntoIterator,
    O4::Item: Clone,
    O5: IntoIterator,
    O5::Item: Clone,
    D0: Fn() -> O0::Item,
    D1: Fn() -> O1::Item,
    D2: Fn() -> O2::Item,
    D3: Fn() -> O3::Item,
    D4: Fn() -> O4::Item,
    D5: Fn() -> O5::Item,
{
    ClampedZip2x6 {
        r0: r0.into_iter(),
        r1: r1.into_iter(),
        o0: o0.into_iter(),
        o1: o1.into_iter(),
        o2: o2.into_iter(),
        o3: o3.into_iter(),
        o4: o4.into_iter(),
        o5: o5.into_iter(),
        o0_default_fn,
        o1_default_fn,
        o2_default_fn,
        o3_default_fn,
        o4_default_fn,
        o5_default_fn,
        o0_latest_value: None,
        o1_latest_value: None,
        o2_latest_value: None,
        o3_latest_value: None,
        o4_latest_value: None,
        o5_latest_value: None,
    }
}

/// Implements a clamped zip iterator combinator with 2 required iterators and 2 optional
/// iterators.
///
/// See [`clamped_zip_2x6`] for more information.
pub struct ClampedZip2x6<R0, R1, O0, O1, O2, O3, O4, O5, D0, D1, D2, D3, D4, D5>
where
    R0: Iterator,
    R1: Iterator,
    O0: Iterator,
    O0::Item: Clone,
    O1: Iterator,
    O1::Item: Clone,
    O2: Iterator,
    O2::Item: Clone,
    O3: Iterator,
    O3::Item: Clone,
    O4: Iterator,
    O4::Item: Clone,
    O5: Iterator,
    O5::Item: Clone,
    D0: Fn() -> O0::Item,
    D1: Fn() -> O1::Item,
    D2: Fn() -> O2::Item,
    D3: Fn() -> O3::Item,
    D4: Fn() -> O4::Item,
    D5: Fn() -> O5::Item,
{
    r0: R0,
    r1: R1,
    o0: O0,
    o1: O1,
    o2: O2,
    o3: O3,
    o4: O4,
    o5: O5,
    o0_default_fn: D0,
    o1_default_fn: D1,
    o2_default_fn: D2,
    o3_default_fn: D3,
    o4_default_fn: D4,
    o5_default_fn: D5,

    o0_latest_value: Option<O0::Item>,
    o1_latest_value: Option<O1::Item>,
    o2_latest_value: Option<O2::Item>,
    o3_latest_value: Option<O3::Item>,
    o4_latest_value: Option<O4::Item>,
    o5_latest_value: Option<O5::Item>,
}

impl<R0, R1, O0, O1, O2, O3, O4, O5, D0, D1, D2, D3, D4, D5> Iterator
    for ClampedZip2x6<R0, R1, O0, O1, O2, O3, O4, O5, D0, D1, D2, D3, D4, D5>
where
    R0: Iterator,
    R1: Iterator,
    O0: Iterator,
    O0::Item: Clone,
    O1: Iterator,
    O1::Item: Clone,
    O2: Iterator,
    O2::Item: Clone,
    O3: Iterator,
    O3::Item: Clone,
    O4: Iterator,
    O4::Item: Clone,
    O5: Iterator,
    O5::Item: Clone,
    D0: Fn() -> O0::Item,
    D1: Fn() -> O1::Item,
    D2: Fn() -> O2::Item,
    D3: Fn() -> O3::Item,
    D4: Fn() -> O4::Item,
    D5: Fn() -> O5::Item,
{
    type Item = (
        R0::Item,
        R1::Item,
        O0::Item,
        O1::Item,
        O2::Item,
        O3::Item,
        O4::Item,
        O5::Item,
    );

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let r0_next = self.r0.next()?;
        let r1_next = self.r1.next()?;
        let o0_next = self.o0.next().or_else(|| self.o0_latest_value.take());
        let o1_next = self.o1.next().or_else(|| self.o1_latest_value.take());
        let o2_next = self.o2.next().or_else(|| self.o2_latest_value.take());
        let o3_next = self.o3.next().or_else(|| self.o3_latest_value.take());
        let o4_next = self.o4.next().or_else(|| self.o4_latest_value.take());
        let o5_next = self.o5.next().or_else(|| self.o5_latest_value.take());

        self.o0_latest_value.clone_from(&o0_next);
        self.o1_latest_value.clone_from(&o1_next);
        self.o2_latest_value.clone_from(&o2_next);
        self.o3_latest_value.clone_from(&o3_next);
        self.o4_latest_value.clone_from(&o4_next);
        self.o5_latest_value.clone_from(&o5_next);

        Some((
            r0_next,
            r1_next,
            o0_next.unwrap_or_else(|| (self.o0_default_fn)()),
            o1_next.unwrap_or_else(|| (self.o1_default_fn)()),
            o2_next.unwrap_or_else(|| (self.o2_default_fn)()),
            o3_next.unwrap_or_else(|| (self.o3_default_fn)()),
            o4_next.unwrap_or_else(|| (self.o4_default_fn)()),
            o5_next.unwrap_or_else(|| (self.o5_default_fn)()),
        ))
    }
}

/// Returns a new [`ClampedZip2x7`] iterator.
///
/// The number of elements in a clamped zip iterator corresponds to the number of elements in the
/// shortest of its required iterators (`r0`, `r1`).
///
/// Optional iterators (`o0`, `o1`, `o2`, `o3`, `o4`, `o5`, `o6`) will repeat their latest values if they happen to be too short
/// to be zipped with the shortest of the required iterators.
///
/// If an optional iterator is not only too short but actually empty, its associated default function
/// (`o0_default_fn`, `o1_default_fn`, `o2_default_fn`, `o3_default_fn`, `o4_default_fn`, `o5_default_fn`, `o6_default_fn`) will be executed and the resulting value repeated as necessary.
pub fn clamped_zip_2x7<R0, R1, O0, O1, O2, O3, O4, O5, O6, D0, D1, D2, D3, D4, D5, D6>(
    r0: R0,
    r1: R1,
    o0: O0,
    o0_default_fn: D0,
    o1: O1,
    o1_default_fn: D1,
    o2: O2,
    o2_default_fn: D2,
    o3: O3,
    o3_default_fn: D3,
    o4: O4,
    o4_default_fn: D4,
    o5: O5,
    o5_default_fn: D5,
    o6: O6,
    o6_default_fn: D6,
) -> ClampedZip2x7<
    R0::IntoIter,
    R1::IntoIter,
    O0::IntoIter,
    O1::IntoIter,
    O2::IntoIter,
    O3::IntoIter,
    O4::IntoIter,
    O5::IntoIter,
    O6::IntoIter,
    D0,
    D1,
    D2,
    D3,
    D4,
    D5,
    D6,
>
where
    R0: IntoIterator,
    R1: IntoIterator,
    O0: IntoIterator,
    O0::Item: Clone,
    O1: IntoIterator,
    O1::Item: Clone,
    O2: IntoIterator,
    O2::Item: Clone,
    O3: IntoIterator,
    O3::Item: Clone,
    O4: IntoIterator,
    O4::Item: Clone,
    O5: IntoIterator,
    O5::Item: Clone,
    O6: IntoIterator,
    O6::Item: Clone,
    D0: Fn() -> O0::Item,
    D1: Fn() -> O1::Item,
    D2: Fn() -> O2::Item,
    D3: Fn() -> O3::Item,
    D4: Fn() -> O4::Item,
    D5: Fn() -> O5::Item,
    D6: Fn() -> O6::Item,
{
    ClampedZip2x7 {
        r0: r0.into_iter(),
        r1: r1.into_iter(),
        o0: o0.into_iter(),
        o1: o1.into_iter(),
        o2: o2.into_iter(),
        o3: o3.into_iter(),
        o4: o4.into_iter(),
        o5: o5.into_iter(),
        o6: o6.into_iter(),
        o0_default_fn,
        o1_default_fn,
        o2_default_fn,
        o3_default_fn,
        o4_default_fn,
        o5_default_fn,
        o6_default_fn,
        o0_latest_value: None,
        o1_latest_value: None,
        o2_latest_value: None,
        o3_latest_value: None,
        o4_latest_value: None,
        o5_latest_value: None,
        o6_latest_value: None,
    }
}

/// Implements a clamped zip iterator combinator with 2 required iterators and 2 optional
/// iterators.
///
/// See [`clamped_zip_2x7`] for more information.
pub struct ClampedZip2x7<R0, R1, O0, O1, O2, O3, O4, O5, O6, D0, D1, D2, D3, D4, D5, D6>
where
    R0: Iterator,
    R1: Iterator,
    O0: Iterator,
    O0::Item: Clone,
    O1: Iterator,
    O1::Item: Clone,
    O2: Iterator,
    O2::Item: Clone,
    O3: Iterator,
    O3::Item: Clone,
    O4: Iterator,
    O4::Item: Clone,
    O5: Iterator,
    O5::Item: Clone,
    O6: Iterator,
    O6::Item: Clone,
    D0: Fn() -> O0::Item,
    D1: Fn() -> O1::Item,
    D2: Fn() -> O2::Item,
    D3: Fn() -> O3::Item,
    D4: Fn() -> O4::Item,
    D5: Fn() -> O5::Item,
    D6: Fn() -> O6::Item,
{
    r0: R0,
    r1: R1,
    o0: O0,
    o1: O1,
    o2: O2,
    o3: O3,
    o4: O4,
    o5: O5,
    o6: O6,
    o0_default_fn: D0,
    o1_default_fn: D1,
    o2_default_fn: D2,
    o3_default_fn: D3,
    o4_default_fn: D4,
    o5_default_fn: D5,
    o6_default_fn: D6,

    o0_latest_value: Option<O0::Item>,
    o1_latest_value: Option<O1::Item>,
    o2_latest_value: Option<O2::Item>,
    o3_latest_value: Option<O3::Item>,
    o4_latest_value: Option<O4::Item>,
    o5_latest_value: Option<O5::Item>,
    o6_latest_value: Option<O6::Item>,
}

impl<R0, R1, O0, O1, O2, O3, O4, O5, O6, D0, D1, D2, D3, D4, D5, D6> Iterator
    for ClampedZip2x7<R0, R1, O0, O1, O2, O3, O4, O5, O6, D0, D1, D2, D3, D4, D5, D6>
where
    R0: Iterator,
    R1: Iterator,
    O0: Iterator,
    O0::Item: Clone,
    O1: Iterator,
    O1::Item: Clone,
    O2: Iterator,
    O2::Item: Clone,
    O3: Iterator,
    O3::Item: Clone,
    O4: Iterator,
    O4::Item: Clone,
    O5: Iterator,
    O5::Item: Clone,
    O6: Iterator,
    O6::Item: Clone,
    D0: Fn() -> O0::Item,
    D1: Fn() -> O1::Item,
    D2: Fn() -> O2::Item,
    D3: Fn() -> O3::Item,
    D4: Fn() -> O4::Item,
    D5: Fn() -> O5::Item,
    D6: Fn() -> O6::Item,
{
    type Item = (
        R0::Item,
        R1::Item,
        O0::Item,
        O1::Item,
        O2::Item,
        O3::Item,
        O4::Item,
        O5::Item,
        O6::Item,
    );

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let r0_next = self.r0.next()?;
        let r1_next = self.r1.next()?;
        let o0_next = self.o0.next().or_else(|| self.o0_latest_value.take());
        let o1_next = self.o1.next().or_else(|| self.o1_latest_value.take());
        let o2_next = self.o2.next().or_else(|| self.o2_latest_value.take());
        let o3_next = self.o3.next().or_else(|| self.o3_latest_value.take());
        let o4_next = self.o4.next().or_else(|| self.o4_latest_value.take());
        let o5_next = self.o5.next().or_else(|| self.o5_latest_value.take());
        let o6_next = self.o6.next().or_else(|| self.o6_latest_value.take());

        self.o0_latest_value.clone_from(&o0_next);
        self.o1_latest_value.clone_from(&o1_next);
        self.o2_latest_value.clone_from(&o2_next);
        self.o3_latest_value.clone_from(&o3_next);
        self.o4_latest_value.clone_from(&o4_next);
        self.o5_latest_value.clone_from(&o5_next);
        self.o6_latest_value.clone_from(&o6_next);

        Some((
            r0_next,
            r1_next,
            o0_next.unwrap_or_else(|| (self.o0_default_fn)()),
            o1_next.unwrap_or_else(|| (self.o1_default_fn)()),
            o2_next.unwrap_or_else(|| (self.o2_default_fn)()),
            o3_next.unwrap_or_else(|| (self.o3_default_fn)()),
            o4_next.unwrap_or_else(|| (self.o4_default_fn)()),
            o5_next.unwrap_or_else(|| (self.o5_default_fn)()),
            o6_next.unwrap_or_else(|| (self.o6_default_fn)()),
        ))
    }
}

/// Returns a new [`ClampedZip2x8`] iterator.
///
/// The number of elements in a clamped zip iterator corresponds to the number of elements in the
/// shortest of its required iterators (`r0`, `r1`).
///
/// Optional iterators (`o0`, `o1`, `o2`, `o3`, `o4`, `o5`, `o6`, `o7`) will repeat their latest values if they happen to be too short
/// to be zipped with the shortest of the required iterators.
///
/// If an optional iterator is not only too short but actually empty, its associated default function
/// (`o0_default_fn`, `o1_default_fn`, `o2_default_fn`, `o3_default_fn`, `o4_default_fn`, `o5_default_fn`, `o6_default_fn`, `o7_default_fn`) will be executed and the resulting value repeated as necessary.
pub fn clamped_zip_2x8<R0, R1, O0, O1, O2, O3, O4, O5, O6, O7, D0, D1, D2, D3, D4, D5, D6, D7>(
    r0: R0,
    r1: R1,
    o0: O0,
    o0_default_fn: D0,
    o1: O1,
    o1_default_fn: D1,
    o2: O2,
    o2_default_fn: D2,
    o3: O3,
    o3_default_fn: D3,
    o4: O4,
    o4_default_fn: D4,
    o5: O5,
    o5_default_fn: D5,
    o6: O6,
    o6_default_fn: D6,
    o7: O7,
    o7_default_fn: D7,
) -> ClampedZip2x8<
    R0::IntoIter,
    R1::IntoIter,
    O0::IntoIter,
    O1::IntoIter,
    O2::IntoIter,
    O3::IntoIter,
    O4::IntoIter,
    O5::IntoIter,
    O6::IntoIter,
    O7::IntoIter,
    D0,
    D1,
    D2,
    D3,
    D4,
    D5,
    D6,
    D7,
>
where
    R0: IntoIterator,
    R1: IntoIterator,
    O0: IntoIterator,
    O0::Item: Clone,
    O1: IntoIterator,
    O1::Item: Clone,
    O2: IntoIterator,
    O2::Item: Clone,
    O3: IntoIterator,
    O3::Item: Clone,
    O4: IntoIterator,
    O4::Item: Clone,
    O5: IntoIterator,
    O5::Item: Clone,
    O6: IntoIterator,
    O6::Item: Clone,
    O7: IntoIterator,
    O7::Item: Clone,
    D0: Fn() -> O0::Item,
    D1: Fn() -> O1::Item,
    D2: Fn() -> O2::Item,
    D3: Fn() -> O3::Item,
    D4: Fn() -> O4::Item,
    D5: Fn() -> O5::Item,
    D6: Fn() -> O6::Item,
    D7: Fn() -> O7::Item,
{
    ClampedZip2x8 {
        r0: r0.into_iter(),
        r1: r1.into_iter(),
        o0: o0.into_iter(),
        o1: o1.into_iter(),
        o2: o2.into_iter(),
        o3: o3.into_iter(),
        o4: o4.into_iter(),
        o5: o5.into_iter(),
        o6: o6.into_iter(),
        o7: o7.into_iter(),
        o0_default_fn,
        o1_default_fn,
        o2_default_fn,
        o3_default_fn,
        o4_default_fn,
        o5_default_fn,
        o6_default_fn,
        o7_default_fn,
        o0_latest_value: None,
        o1_latest_value: None,
        o2_latest_value: None,
        o3_latest_value: None,
        o4_latest_value: None,
        o5_latest_value: None,
        o6_latest_value: None,
        o7_latest_value: None,
    }
}

/// Implements a clamped zip iterator combinator with 2 required iterators and 2 optional
/// iterators.
///
/// See [`clamped_zip_2x8`] for more information.
pub struct ClampedZip2x8<R0, R1, O0, O1, O2, O3, O4, O5, O6, O7, D0, D1, D2, D3, D4, D5, D6, D7>
where
    R0: Iterator,
    R1: Iterator,
    O0: Iterator,
    O0::Item: Clone,
    O1: Iterator,
    O1::Item: Clone,
    O2: Iterator,
    O2::Item: Clone,
    O3: Iterator,
    O3::Item: Clone,
    O4: Iterator,
    O4::Item: Clone,
    O5: Iterator,
    O5::Item: Clone,
    O6: Iterator,
    O6::Item: Clone,
    O7: Iterator,
    O7::Item: Clone,
    D0: Fn() -> O0::Item,
    D1: Fn() -> O1::Item,
    D2: Fn() -> O2::Item,
    D3: Fn() -> O3::Item,
    D4: Fn() -> O4::Item,
    D5: Fn() -> O5::Item,
    D6: Fn() -> O6::Item,
    D7: Fn() -> O7::Item,
{
    r0: R0,
    r1: R1,
    o0: O0,
    o1: O1,
    o2: O2,
    o3: O3,
    o4: O4,
    o5: O5,
    o6: O6,
    o7: O7,
    o0_default_fn: D0,
    o1_default_fn: D1,
    o2_default_fn: D2,
    o3_default_fn: D3,
    o4_default_fn: D4,
    o5_default_fn: D5,
    o6_default_fn: D6,
    o7_default_fn: D7,

    o0_latest_value: Option<O0::Item>,
    o1_latest_value: Option<O1::Item>,
    o2_latest_value: Option<O2::Item>,
    o3_latest_value: Option<O3::Item>,
    o4_latest_value: Option<O4::Item>,
    o5_latest_value: Option<O5::Item>,
    o6_latest_value: Option<O6::Item>,
    o7_latest_value: Option<O7::Item>,
}

impl<R0, R1, O0, O1, O2, O3, O4, O5, O6, O7, D0, D1, D2, D3, D4, D5, D6, D7> Iterator
    for ClampedZip2x8<R0, R1, O0, O1, O2, O3, O4, O5, O6, O7, D0, D1, D2, D3, D4, D5, D6, D7>
where
    R0: Iterator,
    R1: Iterator,
    O0: Iterator,
    O0::Item: Clone,
    O1: Iterator,
    O1::Item: Clone,
    O2: Iterator,
    O2::Item: Clone,
    O3: Iterator,
    O3::Item: Clone,
    O4: Iterator,
    O4::Item: Clone,
    O5: Iterator,
    O5::Item: Clone,
    O6: Iterator,
    O6::Item: Clone,
    O7: Iterator,
    O7::Item: Clone,
    D0: Fn() -> O0::Item,
    D1: Fn() -> O1::Item,
    D2: Fn() -> O2::Item,
    D3: Fn() -> O3::Item,
    D4: Fn() -> O4::Item,
    D5: Fn() -> O5::Item,
    D6: Fn() -> O6::Item,
    D7: Fn() -> O7::Item,
{
    type Item = (
        R0::Item,
        R1::Item,
        O0::Item,
        O1::Item,
        O2::Item,
        O3::Item,
        O4::Item,
        O5::Item,
        O6::Item,
        O7::Item,
    );

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let r0_next = self.r0.next()?;
        let r1_next = self.r1.next()?;
        let o0_next = self.o0.next().or_else(|| self.o0_latest_value.take());
        let o1_next = self.o1.next().or_else(|| self.o1_latest_value.take());
        let o2_next = self.o2.next().or_else(|| self.o2_latest_value.take());
        let o3_next = self.o3.next().or_else(|| self.o3_latest_value.take());
        let o4_next = self.o4.next().or_else(|| self.o4_latest_value.take());
        let o5_next = self.o5.next().or_else(|| self.o5_latest_value.take());
        let o6_next = self.o6.next().or_else(|| self.o6_latest_value.take());
        let o7_next = self.o7.next().or_else(|| self.o7_latest_value.take());

        self.o0_latest_value.clone_from(&o0_next);
        self.o1_latest_value.clone_from(&o1_next);
        self.o2_latest_value.clone_from(&o2_next);
        self.o3_latest_value.clone_from(&o3_next);
        self.o4_latest_value.clone_from(&o4_next);
        self.o5_latest_value.clone_from(&o5_next);
        self.o6_latest_value.clone_from(&o6_next);
        self.o7_latest_value.clone_from(&o7_next);

        Some((
            r0_next,
            r1_next,
            o0_next.unwrap_or_else(|| (self.o0_default_fn)()),
            o1_next.unwrap_or_else(|| (self.o1_default_fn)()),
            o2_next.unwrap_or_else(|| (self.o2_default_fn)()),
            o3_next.unwrap_or_else(|| (self.o3_default_fn)()),
            o4_next.unwrap_or_else(|| (self.o4_default_fn)()),
            o5_next.unwrap_or_else(|| (self.o5_default_fn)()),
            o6_next.unwrap_or_else(|| (self.o6_default_fn)()),
            o7_next.unwrap_or_else(|| (self.o7_default_fn)()),
        ))
    }
}

/// Returns a new [`ClampedZip2x9`] iterator.
///
/// The number of elements in a clamped zip iterator corresponds to the number of elements in the
/// shortest of its required iterators (`r0`, `r1`).
///
/// Optional iterators (`o0`, `o1`, `o2`, `o3`, `o4`, `o5`, `o6`, `o7`, `o8`) will repeat their latest values if they happen to be too short
/// to be zipped with the shortest of the required iterators.
///
/// If an optional iterator is not only too short but actually empty, its associated default function
/// (`o0_default_fn`, `o1_default_fn`, `o2_default_fn`, `o3_default_fn`, `o4_default_fn`, `o5_default_fn`, `o6_default_fn`, `o7_default_fn`, `o8_default_fn`) will be executed and the resulting value repeated as necessary.
pub fn clamped_zip_2x9<
    R0,
    R1,
    O0,
    O1,
    O2,
    O3,
    O4,
    O5,
    O6,
    O7,
    O8,
    D0,
    D1,
    D2,
    D3,
    D4,
    D5,
    D6,
    D7,
    D8,
>(
    r0: R0,
    r1: R1,
    o0: O0,
    o0_default_fn: D0,
    o1: O1,
    o1_default_fn: D1,
    o2: O2,
    o2_default_fn: D2,
    o3: O3,
    o3_default_fn: D3,
    o4: O4,
    o4_default_fn: D4,
    o5: O5,
    o5_default_fn: D5,
    o6: O6,
    o6_default_fn: D6,
    o7: O7,
    o7_default_fn: D7,
    o8: O8,
    o8_default_fn: D8,
) -> ClampedZip2x9<
    R0::IntoIter,
    R1::IntoIter,
    O0::IntoIter,
    O1::IntoIter,
    O2::IntoIter,
    O3::IntoIter,
    O4::IntoIter,
    O5::IntoIter,
    O6::IntoIter,
    O7::IntoIter,
    O8::IntoIter,
    D0,
    D1,
    D2,
    D3,
    D4,
    D5,
    D6,
    D7,
    D8,
>
where
    R0: IntoIterator,
    R1: IntoIterator,
    O0: IntoIterator,
    O0::Item: Clone,
    O1: IntoIterator,
    O1::Item: Clone,
    O2: IntoIterator,
    O2::Item: Clone,
    O3: IntoIterator,
    O3::Item: Clone,
    O4: IntoIterator,
    O4::Item: Clone,
    O5: IntoIterator,
    O5::Item: Clone,
    O6: IntoIterator,
    O6::Item: Clone,
    O7: IntoIterator,
    O7::Item: Clone,
    O8: IntoIterator,
    O8::Item: Clone,
    D0: Fn() -> O0::Item,
    D1: Fn() -> O1::Item,
    D2: Fn() -> O2::Item,
    D3: Fn() -> O3::Item,
    D4: Fn() -> O4::Item,
    D5: Fn() -> O5::Item,
    D6: Fn() -> O6::Item,
    D7: Fn() -> O7::Item,
    D8: Fn() -> O8::Item,
{
    ClampedZip2x9 {
        r0: r0.into_iter(),
        r1: r1.into_iter(),
        o0: o0.into_iter(),
        o1: o1.into_iter(),
        o2: o2.into_iter(),
        o3: o3.into_iter(),
        o4: o4.into_iter(),
        o5: o5.into_iter(),
        o6: o6.into_iter(),
        o7: o7.into_iter(),
        o8: o8.into_iter(),
        o0_default_fn,
        o1_default_fn,
        o2_default_fn,
        o3_default_fn,
        o4_default_fn,
        o5_default_fn,
        o6_default_fn,
        o7_default_fn,
        o8_default_fn,
        o0_latest_value: None,
        o1_latest_value: None,
        o2_latest_value: None,
        o3_latest_value: None,
        o4_latest_value: None,
        o5_latest_value: None,
        o6_latest_value: None,
        o7_latest_value: None,
        o8_latest_value: None,
    }
}

/// Implements a clamped zip iterator combinator with 2 required iterators and 2 optional
/// iterators.
///
/// See [`clamped_zip_2x9`] for more information.
pub struct ClampedZip2x9<
    R0,
    R1,
    O0,
    O1,
    O2,
    O3,
    O4,
    O5,
    O6,
    O7,
    O8,
    D0,
    D1,
    D2,
    D3,
    D4,
    D5,
    D6,
    D7,
    D8,
> where
    R0: Iterator,
    R1: Iterator,
    O0: Iterator,
    O0::Item: Clone,
    O1: Iterator,
    O1::Item: Clone,
    O2: Iterator,
    O2::Item: Clone,
    O3: Iterator,
    O3::Item: Clone,
    O4: Iterator,
    O4::Item: Clone,
    O5: Iterator,
    O5::Item: Clone,
    O6: Iterator,
    O6::Item: Clone,
    O7: Iterator,
    O7::Item: Clone,
    O8: Iterator,
    O8::Item: Clone,
    D0: Fn() -> O0::Item,
    D1: Fn() -> O1::Item,
    D2: Fn() -> O2::Item,
    D3: Fn() -> O3::Item,
    D4: Fn() -> O4::Item,
    D5: Fn() -> O5::Item,
    D6: Fn() -> O6::Item,
    D7: Fn() -> O7::Item,
    D8: Fn() -> O8::Item,
{
    r0: R0,
    r1: R1,
    o0: O0,
    o1: O1,
    o2: O2,
    o3: O3,
    o4: O4,
    o5: O5,
    o6: O6,
    o7: O7,
    o8: O8,
    o0_default_fn: D0,
    o1_default_fn: D1,
    o2_default_fn: D2,
    o3_default_fn: D3,
    o4_default_fn: D4,
    o5_default_fn: D5,
    o6_default_fn: D6,
    o7_default_fn: D7,
    o8_default_fn: D8,

    o0_latest_value: Option<O0::Item>,
    o1_latest_value: Option<O1::Item>,
    o2_latest_value: Option<O2::Item>,
    o3_latest_value: Option<O3::Item>,
    o4_latest_value: Option<O4::Item>,
    o5_latest_value: Option<O5::Item>,
    o6_latest_value: Option<O6::Item>,
    o7_latest_value: Option<O7::Item>,
    o8_latest_value: Option<O8::Item>,
}

impl<R0, R1, O0, O1, O2, O3, O4, O5, O6, O7, O8, D0, D1, D2, D3, D4, D5, D6, D7, D8> Iterator
    for ClampedZip2x9<
        R0,
        R1,
        O0,
        O1,
        O2,
        O3,
        O4,
        O5,
        O6,
        O7,
        O8,
        D0,
        D1,
        D2,
        D3,
        D4,
        D5,
        D6,
        D7,
        D8,
    >
where
    R0: Iterator,
    R1: Iterator,
    O0: Iterator,
    O0::Item: Clone,
    O1: Iterator,
    O1::Item: Clone,
    O2: Iterator,
    O2::Item: Clone,
    O3: Iterator,
    O3::Item: Clone,
    O4: Iterator,
    O4::Item: Clone,
    O5: Iterator,
    O5::Item: Clone,
    O6: Iterator,
    O6::Item: Clone,
    O7: Iterator,
    O7::Item: Clone,
    O8: Iterator,
    O8::Item: Clone,
    D0: Fn() -> O0::Item,
    D1: Fn() -> O1::Item,
    D2: Fn() -> O2::Item,
    D3: Fn() -> O3::Item,
    D4: Fn() -> O4::Item,
    D5: Fn() -> O5::Item,
    D6: Fn() -> O6::Item,
    D7: Fn() -> O7::Item,
    D8: Fn() -> O8::Item,
{
    type Item = (
        R0::Item,
        R1::Item,
        O0::Item,
        O1::Item,
        O2::Item,
        O3::Item,
        O4::Item,
        O5::Item,
        O6::Item,
        O7::Item,
        O8::Item,
    );

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let r0_next = self.r0.next()?;
        let r1_next = self.r1.next()?;
        let o0_next = self.o0.next().or_else(|| self.o0_latest_value.take());
        let o1_next = self.o1.next().or_else(|| self.o1_latest_value.take());
        let o2_next = self.o2.next().or_else(|| self.o2_latest_value.take());
        let o3_next = self.o3.next().or_else(|| self.o3_latest_value.take());
        let o4_next = self.o4.next().or_else(|| self.o4_latest_value.take());
        let o5_next = self.o5.next().or_else(|| self.o5_latest_value.take());
        let o6_next = self.o6.next().or_else(|| self.o6_latest_value.take());
        let o7_next = self.o7.next().or_else(|| self.o7_latest_value.take());
        let o8_next = self.o8.next().or_else(|| self.o8_latest_value.take());

        self.o0_latest_value.clone_from(&o0_next);
        self.o1_latest_value.clone_from(&o1_next);
        self.o2_latest_value.clone_from(&o2_next);
        self.o3_latest_value.clone_from(&o3_next);
        self.o4_latest_value.clone_from(&o4_next);
        self.o5_latest_value.clone_from(&o5_next);
        self.o6_latest_value.clone_from(&o6_next);
        self.o7_latest_value.clone_from(&o7_next);
        self.o8_latest_value.clone_from(&o8_next);

        Some((
            r0_next,
            r1_next,
            o0_next.unwrap_or_else(|| (self.o0_default_fn)()),
            o1_next.unwrap_or_else(|| (self.o1_default_fn)()),
            o2_next.unwrap_or_else(|| (self.o2_default_fn)()),
            o3_next.unwrap_or_else(|| (self.o3_default_fn)()),
            o4_next.unwrap_or_else(|| (self.o4_default_fn)()),
            o5_next.unwrap_or_else(|| (self.o5_default_fn)()),
            o6_next.unwrap_or_else(|| (self.o6_default_fn)()),
            o7_next.unwrap_or_else(|| (self.o7_default_fn)()),
            o8_next.unwrap_or_else(|| (self.o8_default_fn)()),
        ))
    }
}
