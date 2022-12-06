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

pub fn parse_bytes(limit: &str) -> Option<i64> {
    if let Some(kb) = limit.strip_suffix("kB") {
        Some(kb.parse::<i64>().ok()? * 1_000)
    } else if let Some(mb) = limit.strip_suffix("MB") {
        Some(mb.parse::<i64>().ok()? * 1_000_000)
    } else if let Some(gb) = limit.strip_suffix("GB") {
        Some(gb.parse::<i64>().ok()? * 1_000_000_000)
    } else if let Some(tb) = limit.strip_suffix("TB") {
        Some(tb.parse::<i64>().ok()? * 1_000_000_000_000)
    } else {
        None
    }
}

#[test]
fn test_parse_bytes() {
    assert_eq!(parse_bytes("10MB"), Some(10_000_000));
}

// ---

pub fn parse_duration(duration: &str) -> Result<f32, String> {
    fn parse_num(s: &str) -> Result<f32, String> {
        s.parse()
            .map_err(|_ignored| format!("Expected a number, got {s:?}"))
    }

    if let Some(ms) = duration.strip_suffix("ms") {
        Ok(parse_num(ms)? * 1e-3)
    } else if let Some(s) = duration.strip_suffix('s') {
        Ok(parse_num(s)?)
    } else if let Some(s) = duration.strip_suffix('m') {
        Ok(parse_num(s)? * 60.0)
    } else if let Some(s) = duration.strip_suffix('h') {
        Ok(parse_num(s)? * 60.0 * 60.0)
    } else {
        Err(format!(
            "Expected a suffix of 'ms', 's', 'm' or 'h' in string {duration:?}"
        ))
    }
}

#[test]
fn test_parse_duration() {
    assert_eq!(parse_duration("3.2s"), Ok(3.2));
    assert_eq!(parse_duration("250ms"), Ok(0.250));
    assert_eq!(parse_duration("3m"), Ok(3.0 * 60.0));
}
