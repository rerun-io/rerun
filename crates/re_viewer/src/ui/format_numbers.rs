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
