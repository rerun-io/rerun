// TODO: is this the guy we should macro then?
// TODO: can this just be a std::iter::from_fn..?

use std::iter::Peekable;

use re_log_types::{RowId, TimeInt};

// ---

// TODO: generic index?
// TODO: range_zip! macro, itertools style?

// TODO
pub fn range_zip_1x1<Idx, IP0, P0, IC0, C0>(
    p0: IP0,
    c0: IC0,
) -> RangeZip1x1<Idx, IP0::IntoIter, P0, IC0::IntoIter, C0>
where
    Idx: std::cmp::PartialOrd,
    IP0: IntoIterator<Item = (Idx, P0)>,
    IC0: IntoIterator<Item = (Idx, C0)>,
{
    RangeZip1x1 {
        p0: p0.into_iter(),
        c0: c0.into_iter().peekable(),
    }
}

pub struct RangeZip1x1<Idx, IP0, P0, IC0, C0>
where
    Idx: std::cmp::PartialOrd,
    IP0: Iterator<Item = (Idx, P0)>,
    IC0: Iterator<Item = (Idx, C0)>,
{
    p0: IP0,
    c0: Peekable<IC0>,
}

impl<Idx, IP0, P0, IC0, C0> Iterator for RangeZip1x1<Idx, IP0, P0, IC0, C0>
where
    Idx: std::cmp::PartialOrd,
    IP0: Iterator<Item = (Idx, P0)>,
    IC0: Iterator<Item = (Idx, C0)>,
{
    type Item = (Idx, P0, Option<C0>);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let Self { p0, c0 } = self;

        let Some((p0_index, p0_data)) = p0.next() else {
            return None;
        };

        let mut c0_data = None;
        while let Some((_, data)) = c0.next_if(|(index, _)| index <= &p0_index) {
            c0_data = Some(data);
        }

        Some((p0_index, p0_data, c0_data))
    }
}

pub fn range_zip_1x2<Idx, IP0, P0, IC0, C0, IC1, C1>(
    p0: IP0,
    c0: IC0,
    c1: IC1,
) -> RangeZip1x2<Idx, IP0::IntoIter, P0, IC0::IntoIter, C0, IC1::IntoIter, C1>
where
    Idx: std::cmp::PartialOrd,
    IP0: IntoIterator<Item = (Idx, P0)>,
    IC0: IntoIterator<Item = (Idx, C0)>,
    IC1: IntoIterator<Item = (Idx, C1)>,
{
    RangeZip1x2 {
        p0: p0.into_iter(),
        c0: c0.into_iter().peekable(),
        c1: c1.into_iter().peekable(),
    }
}

pub struct RangeZip1x2<Idx, IP0, P0, IC0, C0, IC1, C1>
where
    Idx: std::cmp::PartialOrd,
    IP0: Iterator<Item = (Idx, P0)>,
    IC0: Iterator<Item = (Idx, C0)>,
    IC1: Iterator<Item = (Idx, C1)>,
{
    p0: IP0,
    c0: Peekable<IC0>,
    c1: Peekable<IC1>,
}

impl<Idx, IP0, P0, IC0, C0, IC1, C1> Iterator for RangeZip1x2<Idx, IP0, P0, IC0, C0, IC1, C1>
where
    Idx: std::cmp::PartialOrd,
    IP0: Iterator<Item = (Idx, P0)>,
    IC0: Iterator<Item = (Idx, C0)>,
    IC1: Iterator<Item = (Idx, C1)>,
{
    type Item = (Idx, P0, Option<C0>, Option<C1>);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let Self { p0, c0, c1 } = self;

        let Some((p0_index, p0_data)) = p0.next() else {
            return None;
        };

        let mut c0_data = None;
        while let Some((_, data)) = c0.next_if(|(index, _)| index <= &p0_index) {
            c0_data = Some(data);
        }

        let mut c1_data = None;
        while let Some((_, data)) = c1.next_if(|(index, _)| index <= &p0_index) {
            c1_data = Some(data);
        }

        Some((p0_index, p0_data, c0_data, c1_data))
    }
}

// ---

#[cfg(test)]
mod tests {
    use itertools::Itertools;

    use super::*;

    #[test]
    fn overview_1x1() {
        let p0: Vec<((TimeInt, RowId), u32)> = vec![
            ((9.into(), RowId::ZERO), 90), //
            //
            ((10.into(), RowId::ZERO), 100), //
            //
            ((13.into(), RowId::ZERO.incremented_by(0)), 130), //
            ((13.into(), RowId::ZERO.incremented_by(1)), 131), //
            ((13.into(), RowId::ZERO.incremented_by(2)), 132), //
            ((13.into(), RowId::ZERO.incremented_by(5)), 135), //
            //
            ((14.into(), RowId::ZERO), 140), //
        ];

        let c0: Vec<((TimeInt, RowId), &'static str)> = vec![
            ((10.into(), RowId::ZERO.incremented_by(1)), "101"), //
            ((10.into(), RowId::ZERO.incremented_by(2)), "102"), //
            ((10.into(), RowId::ZERO.incremented_by(3)), "103"), //
            //
            ((11.into(), RowId::ZERO), "110"), //
            //
            ((12.into(), RowId::ZERO), "120"), //
            //
            ((13.into(), RowId::ZERO.incremented_by(1)), "131"), //
            ((13.into(), RowId::ZERO.incremented_by(2)), "132"), //
            ((13.into(), RowId::ZERO.incremented_by(4)), "134"), //
            ((13.into(), RowId::ZERO.incremented_by(6)), "136"), //
        ];

        let expected: Vec<((TimeInt, RowId), u32, Option<&'static str>)> = vec![
            ((9.into(), RowId::ZERO), 90, None), //
            //
            ((10.into(), RowId::ZERO), 100, None), //
            //
            ((13.into(), RowId::ZERO.incremented_by(0)), 130, Some("120")), //
            ((13.into(), RowId::ZERO.incremented_by(1)), 131, Some("131")), //
            ((13.into(), RowId::ZERO.incremented_by(2)), 132, Some("132")), //
            ((13.into(), RowId::ZERO.incremented_by(5)), 135, Some("134")), //
            //
            ((14.into(), RowId::ZERO), 140, Some("136")), //
        ];
        let got = range_zip_1x1(p0, c0).collect_vec();

        similar_asserts::assert_eq!(expected, got);
    }
}
