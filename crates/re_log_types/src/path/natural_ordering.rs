//! Implement natural ordering for strings, so that "file5" < "file10".
//!
//! Crates considered:
//! * `human-sort`: <https://github.com/paradakh/human-sort/blob/master/src/lib.rs> - overflows on large integers
//! * `lexical-sort`: <https://lib.rs/crates/lexical-sort> - comes with a huge unicode->ascii table
//! * `natord`: <https://docs.rs/natord/latest/natord/> - the one we're using

use std::cmp::Ordering;

/// Natural ordering for strings, so that "file5" < "file10".
pub fn compare(a: &str, b: &str) -> Ordering {
    natord::compare_iter(
        a.chars(),
        b.chars(),
        |_| false,
        |&l, &r| compare_chars(l, r),
        |&c| c.to_digit(10).map(|v| v as isize),
    )
}

// Ignore case when ordering, so that `a < B < b < c`
fn compare_chars(a: char, b: char) -> Ordering {
    let al = a.to_ascii_lowercase();
    let bl = b.to_ascii_lowercase();

    if al == bl {
        a.cmp(&b)
    } else {
        al.cmp(&bl)
    }
}

#[test]
fn test_natural_ordering() {
    fn check_total_order(strs: &[&str]) {
        fn ordering_str(ord: Ordering) -> &'static str {
            match ord {
                Ordering::Greater => ">",
                Ordering::Equal => "=",
                Ordering::Less => "<",
            }
        }

        for (i, &x) in strs.iter().enumerate() {
            for (j, &y) in strs.iter().enumerate() {
                assert!(
                    compare(x, y) == i.cmp(&j),
                    "Got {x:?} {} {y:?}; expected {x:?} {} {y:?}",
                    ordering_str(i.cmp(&j)),
                    ordering_str(compare(x, y))
                );
            }
        }
    }

    check_total_order(&["10", "a", "aa", "b", "c"]);
    check_total_order(&["a", "a0", "a1", "a1a", "a1b", "a2", "a10", "a20"]);
    check_total_order(&["1.001", "1.002", "1.010", "1.02", "1.1", "1.3"]);
    check_total_order(&["a 2", "a2"]);
    check_total_order(&["a", "B", "b", "c"]);
    assert!(compare("a", "a") == Ordering::Equal);
    assert!(compare("a", "A") != Ordering::Equal);
}
