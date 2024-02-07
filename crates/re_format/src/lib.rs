//! Miscellaneous tools to format and parse numbers, durations, etc.

#[cfg(feature = "arrow")]
pub mod arrow;

mod time;

pub use time::next_grid_tick_magnitude_ns;

// --- Numbers ---

/// Pretty format a number by using thousands separators for readability.
pub fn format_number(number: usize) -> String {
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
fn test_format_number() {
    assert_eq!(format_number(42), "42");
    assert_eq!(format_number(999), "999");
    assert_eq!(format_number(1_000), "1 000");
    assert_eq!(format_number(123_456), "123 456");
    assert_eq!(format_number(1_234_567), "1 234 567");
}

/// Format a number with a decent number of decimals.
pub fn format_f64(value: f64) -> String {
    let is_integer = value.round() == value;
    if is_integer {
        return format!("{value:.0}");
    }

    let magnitude = value.abs().log10();
    let num_decimals = (3.5 - magnitude).round().max(1.0) as usize;
    format!("{value:.num_decimals$}")
}

/// Format a number with a decent number of decimals.
pub fn format_f32(value: f32) -> String {
    format_f64(value as f64)
}

#[test]
fn test_format_float() {
    assert_eq!(format_f64(42.0), "42");
    assert_eq!(format_f64(123_456_789.0), "123456789");
    assert_eq!(format_f64(123_456_789.123_45), "123456789.1");
    assert_eq!(format_f64(0.0000123456789), "0.00001235");
    assert_eq!(format_f64(0.123456789), "0.1235");
    assert_eq!(format_f64(1.23456789), "1.235");
    assert_eq!(format_f64(12.3456789), "12.35");
    assert_eq!(format_f64(123.456789), "123.5");
    assert_eq!(format_f64(1234.56789), "1234.6");
    assert_eq!(format_f64(12345.6789), "12345.7");
    assert_eq!(format_f64(78.4321), "78.43");
}

/// Pretty format a large number by using SI notation (base 10), e.g.
///
/// ```
/// # use re_format::approximate_large_number;
/// assert_eq!(approximate_large_number(123 as _), "123");
/// assert_eq!(approximate_large_number(12_345 as _), "12k");
/// assert_eq!(approximate_large_number(1_234_567 as _), "1.2M");
/// assert_eq!(approximate_large_number(123_456_789 as _), "123M");
/// ```
///
/// Prefer to use [`format_number`], which outputs an exact string,
/// while still being readable thanks to half-width spaces used as thousands-separators.
pub fn approximate_large_number(number: f64) -> String {
    if number < 0.0 {
        return format!("-{}", approximate_large_number(-number));
    }

    if number < 1000.0 {
        format!("{number:.0}")
    } else if number < 1_000_000.0 {
        let decimals = (number < 10_000.0) as usize;
        format!("{:.*}k", decimals, number / 1_000.0)
    } else if number < 1_000_000_000.0 {
        let decimals = (number < 10_000_000.0) as usize;
        format!("{:.*}M", decimals, number / 1_000_000.0)
    } else {
        let decimals = (number < 10_000_000_000.0) as usize;
        format!("{:.*}G", decimals, number / 1_000_000_000.0)
    }
}

#[test]
fn test_format_large_number() {
    let test_cases = [
        (999.0, "999"),
        (1000.0, "1.0k"),
        (1001.0, "1.0k"),
        (999_999.0, "1000k"),
        (1_000_000.0, "1.0M"),
        (999_999_999.0, "1000M"),
        (1_000_000_000.0, "1.0G"),
        (999_999_999_999.0, "1000G"),
        (1_000_000_000_000.0, "1000G"),
        (123.0, "123"),
        (12_345.0, "12k"),
        (1_234_567.0, "1.2M"),
        (123_456_789.0, "123M"),
    ];

    for (value, expected) in test_cases {
        assert_eq!(expected, approximate_large_number(value));
    }
}

// --- Bytes ---

/// Pretty format a number of bytes by using SI notation (base2), e.g.
///
/// ```
/// # use re_format::format_bytes;
/// assert_eq!(format_bytes(123.0), "123 B");
/// assert_eq!(format_bytes(12_345.0), "12.1 KiB");
/// assert_eq!(format_bytes(1_234_567.0), "1.2 MiB");
/// assert_eq!(format_bytes(123_456_789.0), "118 MiB");
/// ```
pub fn format_bytes(number_of_bytes: f64) -> String {
    if number_of_bytes < 0.0 {
        return format!("-{}", format_bytes(-number_of_bytes));
    }

    if number_of_bytes < 10.0_f64.exp2() {
        format!("{number_of_bytes:.0} B")
    } else if number_of_bytes < 20.0_f64.exp2() {
        let decimals = (10.0 * number_of_bytes < 20.0_f64.exp2()) as usize;
        format!("{:.*} KiB", decimals, number_of_bytes / 10.0_f64.exp2())
    } else if number_of_bytes < 30.0_f64.exp2() {
        let decimals = (10.0 * number_of_bytes < 30.0_f64.exp2()) as usize;
        format!("{:.*} MiB", decimals, number_of_bytes / 20.0_f64.exp2())
    } else {
        let decimals = (10.0 * number_of_bytes < 40.0_f64.exp2()) as usize;
        format!("{:.*} GiB", decimals, number_of_bytes / 30.0_f64.exp2())
    }
}

#[test]
fn test_format_bytes() {
    let test_cases = [
        (999.0, "999 B"),
        (1000.0, "1000 B"),
        (1001.0, "1001 B"),
        (1023.0, "1023 B"),
        (1024.0, "1.0 KiB"),
        (1025.0, "1.0 KiB"),
        (1024.0 * 1.2345, "1.2 KiB"),
        (1024.0 * 12.345, "12.3 KiB"),
        (1024.0 * 123.45, "123 KiB"),
        (1024f64.powi(2) - 1.0, "1024 KiB"),
        (1024f64.powi(2) + 0.0, "1.0 MiB"),
        (1024f64.powi(2) + 1.0, "1.0 MiB"),
        (1024f64.powi(3) - 1.0, "1024 MiB"),
        (1024f64.powi(3) + 0.0, "1.0 GiB"),
        (1024f64.powi(3) + 1.0, "1.0 GiB"),
        (1.2345 * 30.0_f64.exp2(), "1.2 GiB"),
        (12.345 * 30.0_f64.exp2(), "12.3 GiB"),
        (123.45 * 30.0_f64.exp2(), "123 GiB"),
        (1024f64.powi(4) - 1.0, "1024 GiB"),
        (1024f64.powi(4) + 0.0, "1024 GiB"),
        (1024f64.powi(4) + 1.0, "1024 GiB"),
        (123.0, "123 B"),
        (12_345.0, "12.1 KiB"),
        (1_234_567.0, "1.2 MiB"),
        (123_456_789.0, "118 MiB"),
    ];

    for (value, expected) in test_cases {
        assert_eq!(format_bytes(value), expected);
    }
}

pub fn parse_bytes_base10(bytes: &str) -> Option<i64> {
    if let Some(kb) = bytes.strip_suffix("kB") {
        Some(kb.parse::<i64>().ok()? * 1_000)
    } else if let Some(mb) = bytes.strip_suffix("MB") {
        Some(mb.parse::<i64>().ok()? * 1_000_000)
    } else if let Some(gb) = bytes.strip_suffix("GB") {
        Some(gb.parse::<i64>().ok()? * 1_000_000_000)
    } else if let Some(tb) = bytes.strip_suffix("TB") {
        Some(tb.parse::<i64>().ok()? * 1_000_000_000_000)
    } else if let Some(b) = bytes.strip_suffix('B') {
        Some(b.parse::<i64>().ok()?)
    } else {
        None
    }
}

#[test]
fn test_parse_bytes_base10() {
    let test_cases = [
        ("999B", 999),
        ("1000B", 1_000),
        ("1kB", 1_000),
        ("1000kB", 1_000_000),
        ("1MB", 1_000_000),
        ("1000MB", 1_000_000_000),
        ("1GB", 1_000_000_000),
        ("1000GB", 1_000_000_000_000),
        ("1TB", 1_000_000_000_000),
        ("1000TB", 1_000_000_000_000_000),
        ("123B", 123),
        ("12kB", 12_000),
        ("123MB", 123_000_000),
    ];
    for (value, expected) in test_cases {
        assert_eq!(Some(expected), parse_bytes_base10(value));
    }
}

pub fn parse_bytes_base2(bytes: &str) -> Option<i64> {
    if let Some(kb) = bytes.strip_suffix("KiB") {
        Some(kb.parse::<i64>().ok()? * 1024)
    } else if let Some(mb) = bytes.strip_suffix("MiB") {
        Some(mb.parse::<i64>().ok()? * 1024 * 1024)
    } else if let Some(gb) = bytes.strip_suffix("GiB") {
        Some(gb.parse::<i64>().ok()? * 1024 * 1024 * 1024)
    } else if let Some(tb) = bytes.strip_suffix("TiB") {
        Some(tb.parse::<i64>().ok()? * 1024 * 1024 * 1024 * 1024)
    } else if let Some(b) = bytes.strip_suffix('B') {
        Some(b.parse::<i64>().ok()?)
    } else {
        None
    }
}

#[test]
fn test_parse_bytes_base2() {
    let test_cases = [
        ("999B", 999),
        ("1023B", 1_023),
        ("1024B", 1_024),
        ("1KiB", 1_024),
        ("1000KiB", 1_000 * 1024),
        ("1MiB", 1024 * 1024),
        ("1000MiB", 1_000 * 1024 * 1024),
        ("1GiB", 1024 * 1024 * 1024),
        ("1000GiB", 1_000 * 1024 * 1024 * 1024),
        ("1TiB", 1024 * 1024 * 1024 * 1024),
        ("1000TiB", 1_000 * 1024 * 1024 * 1024 * 1024),
        ("123B", 123),
        ("12KiB", 12 * 1024),
        ("123MiB", 123 * 1024 * 1024),
    ];
    for (value, expected) in test_cases {
        assert_eq!(Some(expected), parse_bytes_base2(value));
    }
}

pub fn parse_bytes(bytes: &str) -> Option<i64> {
    parse_bytes_base10(bytes).or_else(|| parse_bytes_base2(bytes))
}

#[test]
fn test_parse_bytes() {
    let test_cases = [
        // base10
        ("999B", 999),
        ("1000B", 1_000),
        ("1kB", 1_000),
        ("1000kB", 1_000_000),
        ("1MB", 1_000_000),
        ("1000MB", 1_000_000_000),
        ("1GB", 1_000_000_000),
        ("1000GB", 1_000_000_000_000),
        ("1TB", 1_000_000_000_000),
        ("1000TB", 1_000_000_000_000_000),
        ("123B", 123),
        ("12kB", 12_000),
        ("123MB", 123_000_000),
        // base2
        ("999B", 999),
        ("1023B", 1_023),
        ("1024B", 1_024),
        ("1KiB", 1_024),
        ("1000KiB", 1_000 * 1024),
        ("1MiB", 1024 * 1024),
        ("1000MiB", 1_000 * 1024 * 1024),
        ("1GiB", 1024 * 1024 * 1024),
        ("1000GiB", 1_000 * 1024 * 1024 * 1024),
        ("1TiB", 1024 * 1024 * 1024 * 1024),
        ("1000TiB", 1_000 * 1024 * 1024 * 1024 * 1024),
        ("123B", 123),
        ("12KiB", 12 * 1024),
        ("123MiB", 123 * 1024 * 1024),
    ];
    for (value, expected) in test_cases {
        assert_eq!(Some(expected), parse_bytes(value));
    }
}

// --- Durations ---

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
