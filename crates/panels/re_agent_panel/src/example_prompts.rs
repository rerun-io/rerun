//! Picks which example prompts to offer in an empty conversation.

use std::sync::LazyLock;

/// How many example prompts to offer at once.
const NUM_SHOWN: usize = 3;

/// Fixed for the lifetime of the viewer, so the offered prompts don't change under the user,
/// but differ between restarts.
static SEED: LazyLock<u64> = LazyLock::new(|| {
    web_time::SystemTime::now()
        .duration_since(web_time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos() as u64)
});

/// Picks a few of the `prompts`, favoring the ones early in the list.
///
/// `prompts` must be sorted best-first. The picks keep that order.
pub fn sample(prompts: &[&'static str]) -> Vec<&'static str> {
    sample_with_seed(*SEED, prompts)
}

/// Weighted sampling without replacement (Efraimidis-Spirakis), with weight `1 / (i + 1)`
/// for the prompt at index `i`.
fn sample_with_seed(seed: u64, prompts: &[&'static str]) -> Vec<&'static str> {
    let mut keyed: Vec<(f64, usize)> = (0..prompts.len())
        .map(|i| {
            let uniform = unit_interval(splitmix64(seed ^ splitmix64(i as u64)));
            (-uniform.ln() * (i + 1) as f64, i)
        })
        .collect();
    keyed.sort_by(|a, b| a.0.total_cmp(&b.0));

    let mut picked: Vec<usize> = keyed.into_iter().take(NUM_SHOWN).map(|(_, i)| i).collect();
    picked.sort_unstable();
    picked.into_iter().map(|i| prompts[i]).collect()
}

/// Maps to `(0, 1]`, so that `ln` stays finite.
fn unit_interval(x: u64) -> f64 {
    ((x >> 11) as f64 + 1.0) / (1u64 << 53) as f64
}

fn splitmix64(x: u64) -> u64 {
    let mut z = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;

    const PROMPTS: [&str; 6] = ["a", "b", "c", "d", "e", "f"];

    #[test]
    fn short_list_is_shown_whole() {
        assert_eq!(sample_with_seed(7, &[]), Vec::<&str>::new());
        assert_eq!(sample_with_seed(7, &["a", "b"]), ["a", "b"]);
    }

    #[test]
    fn picks_are_distinct_and_in_list_order() {
        for seed in 0..1000 {
            let picked = sample_with_seed(seed, &PROMPTS);
            assert_eq!(picked.len(), NUM_SHOWN);
            assert!(picked.is_sorted(), "{picked:?}");
            assert!(picked.windows(2).all(|pair| pair[0] != pair[1]));
        }
    }

    #[test]
    fn same_seed_same_picks() {
        assert_eq!(
            sample_with_seed(42, &PROMPTS),
            sample_with_seed(42, &PROMPTS)
        );
    }

    #[test]
    fn earlier_prompts_are_favored() {
        let count = |prompt| {
            (0..10_000)
                .filter(|&seed| sample_with_seed(seed, &PROMPTS).contains(&prompt))
                .count()
        };
        let (first, last) = (count("a"), count("f"));
        assert!(last * 2 < first, "first: {first}, last: {last}");
        assert!(0 < last, "every prompt should get a chance");
    }
}
