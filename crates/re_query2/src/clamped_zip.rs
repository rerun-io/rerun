// TODO: is this the guy we should macro then?

// ---

pub fn clamped_zip_1x1<P0, I0, D0>(
    it0: P0,
    it1: I0,
    it1_default_fn: D0,
) -> ClampedZip1x1<P0::IntoIter, I0::IntoIter, D0>
where
    P0: IntoIterator,
    I0: IntoIterator,
    I0::Item: Clone,
    D0: Fn() -> I0::Item,
{
    ClampedZip1x1 {
        it0: it0.into_iter(),
        it1: it1.into_iter(),
        last_it1_value: None,
        default_it1_value: it1_default_fn,
    }
}

pub fn clamped_zip_1x2<P0, I0, D0, I1, D1>(
    it0: P0,
    it1: I0,
    it1_default_fn: D0,
    it2: I1,
    it2_default_fn: D1,
) -> ClampedZip1x2<P0::IntoIter, I0::IntoIter, D0, I1::IntoIter, D1>
where
    P0: IntoIterator,
    I0: IntoIterator,
    I0::Item: Clone,
    D0: Fn() -> I0::Item,
    I1: IntoIterator,
    I1::Item: Clone,
    D1: Fn() -> I1::Item,
{
    ClampedZip1x2 {
        it0: it0.into_iter(),
        it1: it1.into_iter(),
        it2: it2.into_iter(),
        last_it1_value: None,
        default_it1_value: it1_default_fn,
        last_it2_value: None,
        default_it2_value: it2_default_fn,
    }
}

// ---

// TODO
pub struct ClampedZip1x1<P0, I0, D0>
where
    P0: Iterator,
    I0: Iterator,
    I0::Item: Clone,
    D0: Fn() -> I0::Item,
{
    it0: P0,
    it1: I0,

    last_it1_value: Option<I0::Item>,
    default_it1_value: D0,
}

impl<P0, I0, D0> Iterator for ClampedZip1x1<P0, I0, D0>
where
    P0: Iterator,
    I0: Iterator,
    I0::Item: Clone,
    D0: Fn() -> I0::Item,
{
    type Item = (P0::Item, I0::Item);

    fn next(&mut self) -> Option<Self::Item> {
        let it0_next = self.it0.next();
        let it1_next = self.it1.next().or(self.last_it1_value.take());

        self.last_it1_value = it1_next.clone();

        it0_next.map(|it0_next| {
            (
                it0_next,
                it1_next.unwrap_or_else(|| (self.default_it1_value)()),
            )
        })
    }
}

// TODO
pub struct ClampedZip1x2<P0, I0, D0, I1, D1>
where
    P0: Iterator,
    I0: Iterator,
    I0::Item: Clone,
    D0: Fn() -> I0::Item,
    I1: Iterator,
    I1::Item: Clone,
    D1: Fn() -> I1::Item,
{
    it0: P0,
    it1: I0,
    it2: I1,

    last_it1_value: Option<I0::Item>,
    default_it1_value: D0,

    last_it2_value: Option<I1::Item>,
    default_it2_value: D1,
}

impl<P0, I0, D0, I1, D1> Iterator for ClampedZip1x2<P0, I0, D0, I1, D1>
where
    P0: Iterator,
    I0: Iterator,
    I0::Item: Clone,
    D0: Fn() -> I0::Item,
    I1: Iterator,
    I1::Item: Clone,
    D1: Fn() -> I1::Item,
{
    type Item = (P0::Item, I0::Item, I1::Item);

    fn next(&mut self) -> Option<Self::Item> {
        let it0_next = self.it0.next();
        let it1_next = self.it1.next().or(self.last_it1_value.take());
        let it2_next = self.it2.next().or(self.last_it2_value.take());

        self.last_it1_value = it1_next.clone();
        self.last_it2_value = it2_next.clone();

        it0_next.map(|it0_next| {
            (
                it0_next,
                it1_next.unwrap_or_else(|| (self.default_it1_value)()),
                it2_next.unwrap_or_else(|| (self.default_it2_value)()),
            )
        })
    }
}

// ---

// TODO: mark as test
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it0_is_empty_it1_is_empty() {
        let it0 = std::iter::empty::<u32>();
        let it1 = (0..).map(|n| n.to_string());

        let expected: Vec<(u32, String)> = vec![];
        let got = clamped_zip_1x1(it0, it1, String::new).collect::<Vec<_>>();

        similar_asserts::assert_eq!(expected, got);
    }

    #[test]
    fn it0_and_it1_are_matched() {
        let it0 = 0..20u32;
        let it1 = (0..20).map(|n| n.to_string());

        let expected: Vec<(u32, String)> = (0..20u32).map(|n| (n, n.to_string())).collect();
        let got = clamped_zip_1x1(it0, it1, String::new).collect::<Vec<_>>();

        similar_asserts::assert_eq!(expected, got);
    }

    #[test]
    fn it0_is_shorter() {
        let it0 = 0..10u32;
        let it1 = (0..20).map(|n| n.to_string());

        let expected: Vec<(u32, String)> = (0..10u32).map(|n| (n, n.to_string())).collect();
        let got = clamped_zip_1x1(it0, it1, String::new).collect::<Vec<_>>();

        similar_asserts::assert_eq!(expected, got);
    }

    #[test]
    fn it0_is_longer() {
        let it0 = 0..30u32;
        let it1 = (0..20).map(|n| n.to_string());

        let expected: Vec<(u32, String)> = (0..30u32)
            .map(|n| (n, u32::min(n, 19).to_string()))
            .collect();
        let got = clamped_zip_1x1(it0, it1, String::new).collect::<Vec<_>>();

        similar_asserts::assert_eq!(expected, got);
    }

    #[test]
    fn it0_is_longer_and_it1_is_empty() {
        let it0 = 0..10u32;
        let it1 = std::iter::empty();

        let expected: Vec<(u32, String)> = (0..10u32).map(|n| (n, "hey".to_owned())).collect();
        let got = clamped_zip_1x1(it0, it1, || "hey".to_owned()).collect::<Vec<_>>();

        similar_asserts::assert_eq!(expected, got);
    }
}
