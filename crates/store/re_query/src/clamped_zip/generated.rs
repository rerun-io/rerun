// This file was generated using `cargo r -p re_query --all-features --bin clamped_zip > crates/store/re_query/src/clamped_zip/generated.rs && cargo fmt`.
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
