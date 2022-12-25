use re_int_histogram::better::IntHistogram;

fn main() {
    let mut set = IntHistogram::default();
    for i in 0..=100 {
        assert_eq!(set.total_count(), i);
        assert_eq!(set.range_count(-10000..10000), i);
        assert_eq!(set.range_count(0..5), i.min(5));
        let key = i as i64;
        set.increment(key, 1);
        assert_eq!(set.range_count(0..=0), 1);
    }
    println!("All tests passed!");
}
