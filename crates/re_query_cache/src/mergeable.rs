// TODO: get rid of the type and just use QueryCaches instead?
trait Mergeable {
    fn range(&self) -> RangeInclusive<i64>;

    fn merge(self, rhs: Self) -> Self;
}

fn find_and_merge<T: Mergeable>(map: &mut BTreeMap<i64, T>, mut added: T) {
    let added_range = added.range();

    let mut kept = Vec::new();
    for (bucket_start, bucket) in std::mem::take(map).into_iter() {
        let bucket_range = bucket.range();

        // NOTE: closures because lazy checks.

        // E.g. b1=1..=3 b2=0..=2
        let min_bound_overlaps = || {
            bucket_range.start() <= added_range.start() && bucket_range.end() >= added_range.start()
        };

        // E.g. b1=1..=3 b2=2..=3
        let max_bound_overlaps =
            || bucket_range.start() <= added_range.end() && bucket_range.end() >= added_range.end();

        // E.g. b1=1..=3 b2=4..=5 or b1=2..=3 b2=0..=1
        let connected = || {
            added_range.end().saturating_add(1) == *bucket_range.start()
                || bucket_range.end().saturating_add(1) == *added_range.start()
        };

        if min_bound_overlaps() || max_bound_overlaps() || connected() {
            added = added.merge(bucket);
        } else {
            kept.push((bucket_start, bucket));
        }
    }

    *map = kept
        .into_iter()
        .chain([(*added.range().start(), added)])
        .collect();
}

#[test]
fn find_and_merge_xxx() {
    #[derive(Debug, Clone)]
    struct Ints {
        range: RangeInclusive<i64>,
        set: BTreeSet<u32>,
    }

    impl Ints {
        fn new(range: RangeInclusive<i64>, values: impl IntoIterator<Item = u32>) -> Self {
            Self {
                range,
                set: values.into_iter().collect(),
            }
        }
    }

    impl Mergeable for Ints {
        fn range(&self) -> RangeInclusive<i64> {
            self.range.clone()
        }

        fn merge(self, rhs: Self) -> Self {
            let start = i64::min(*self.range.start(), *rhs.range.start());
            let end = i64::max(*self.range.end(), *rhs.range.end());
            let range = start..=end;

            let set = self.set.into_iter().chain(rhs.set).collect();

            Self { range, set }
        }
    }

    // TODO

    let mut map: BTreeMap<i64, Ints> = Default::default();
    dbg!(&map);

    find_and_merge(&mut map, Ints::new(1..=1, [1]));
    dbg!(&map);

    // Expected: no-op.
    // TODO: wtf is this even supposed to do? should be ignored, right?
    find_and_merge(&mut map, Ints::new(1..=1, [2]));
    dbg!(&map);

    find_and_merge(&mut map, Ints::new(4..=5, [4, 5]));
    dbg!(&map);

    // Expected: one big bucket.
    find_and_merge(&mut map, Ints::new(2..=3, [2, 3]));
    dbg!(&map);
}
