//! Miscellaneous formatting tools.

// ---

/// Using thousands separators for readability.
pub fn format_usize(number: usize) -> String {
    let number = number.to_string();
    let mut chars = number.chars().rev().peekable();

    let mut result = vec![];
    while chars.peek().is_some() {
        if !result.is_empty() {
            // thousands-deliminator:
            let thin_space = '\u{2009}'; // https://en.wikipedia.org/wiki/Thin_space
            result.push(thin_space);
        }
        for _ in 0..3 {
            if let Some(c) = chars.next() {
                result.push(c);
            }
        }
    }

    result.reverse();
    result.into_iter().collect()
}

#[test]
fn test_format_usize() {
    assert_eq!(format_usize(42), "42");
    assert_eq!(format_usize(999), "999");
    assert_eq!(format_usize(1_000), "1 000");
    assert_eq!(format_usize(123_456), "123 456");
    assert_eq!(format_usize(1_234_567), "1 234 567");
}

// ---

/// Pretty format bytes, e.g.
///
/// ```
/// # use re_memory::util::format_bytes;
/// assert_eq!(format_bytes(123 as _), "123 B");
/// assert_eq!(format_bytes(12_345 as _), "12 kB");
/// assert_eq!(format_bytes(1_234_567 as _), "1.2 MB");
/// assert_eq!(format_bytes(123_456_789 as _), "123 MB");
/// ```
pub fn format_bytes(number_of_bytes: f64) -> String {
    if number_of_bytes < 0.0 {
        return format!("-{}", format_bytes(-number_of_bytes));
    }

    if number_of_bytes < 1000.0 {
        format!("{:.0} B", number_of_bytes)
    } else if number_of_bytes < 1_000_000.0 {
        let decimals = (number_of_bytes < 10_000.0) as usize;
        format!("{:.*} kB", decimals, number_of_bytes / 1_000.0)
    } else if number_of_bytes < 1_000_000_000.0 {
        let decimals = (number_of_bytes < 10_000_000.0) as usize;
        format!("{:.*} MB", decimals, number_of_bytes / 1_000_000.0)
    } else {
        let decimals = (number_of_bytes < 10_000_000_000.0) as usize;
        format!("{:.*} GB", decimals, number_of_bytes / 1_000_000_000.0)
    }
}
