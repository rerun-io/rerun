//! Miscellaneous formatting tools.

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

/// Pretty format a large number by using SI notation (base 10), e.g.
///
/// ```
/// # use re_format::format_large_number;
/// assert_eq!(format_large_number(123 as _), "123");
/// assert_eq!(format_large_number(12_345 as _), "12k");
/// assert_eq!(format_large_number(1_234_567 as _), "1.2M");
/// assert_eq!(format_large_number(123_456_789 as _), "123M");
/// ```
pub fn format_large_number(number: f64) -> String {
    if number < 0.0 {
        return format!("-{}", format_large_number(-number));
    }

    if number < 1000.0 {
        format!("{:.0}", number)
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
        assert_eq!(expected, format_large_number(value));
    }
}

// --- Bytes ---

/// Pretty format a number of bytes by using SI notation (base2), e.g.
///
/// ```
/// # use re_format::format_bytes;
/// assert_eq!(format_bytes(123.0), "123 B");
/// assert_eq!(format_bytes(12_345.0), "12.1 kiB");
/// assert_eq!(format_bytes(1_234_567.0), "1.2 MiB");
/// assert_eq!(format_bytes(123_456_789.0), "117.7 MiB");
/// ```
pub fn format_bytes(number_of_bytes: f64) -> String {
    if number_of_bytes < 0.0 {
        return format!("-{}", format_bytes(-number_of_bytes));
    }

    if number_of_bytes < (1 << 10) as f64 {
        format!("{:.0} B", number_of_bytes)
    } else if number_of_bytes < (1 << 20) as f64 {
        let decimals = (number_of_bytes < (1 << 18) as f64) as usize;
        format!("{:.*} kiB", decimals, number_of_bytes / (1 << 10) as f64)
    } else if number_of_bytes < (1 << 30) as f64 {
        let decimals = (number_of_bytes < (1 << 28) as f64) as usize;
        format!("{:.*} MiB", decimals, number_of_bytes / (1 << 20) as f64)
    } else {
        let decimals = (number_of_bytes < (1 << 31) as f64) as usize;
        format!("{:.*} GiB", decimals, number_of_bytes / (1 << 30) as f64)
    }
}

#[test]
fn test_format_bytes() {
    let test_cases = [
        (999.0, "999 B"),
        (1000.0, "1000 B"),
        (1001.0, "1001 B"),
        (1023.0, "1023 B"),
        (1024.0, "1.0 kiB"),
        (1025.0, "1.0 kiB"),
        (1024f64.powi(2) - 1.0, "1024 kiB"),
        (1024f64.powi(2) + 0.0, "1.0 MiB"),
        (1024f64.powi(2) + 1.0, "1.0 MiB"),
        (1024f64.powi(3) - 1.0, "1024 MiB"),
        (1024f64.powi(3) + 0.0, "1 GiB"),
        (1024f64.powi(3) + 1.0, "1 GiB"),
        (1024f64.powi(4) - 1.0, "1024 GiB"),
        (1024f64.powi(4) + 0.0, "1024 GiB"),
        (1024f64.powi(4) + 1.0, "1024 GiB"),
        (123.0, "123 B"),
        (12_345.0, "12.1 kiB"),
        (1_234_567.0, "1.2 MiB"),
        (123_456_789.0, "117.7 MiB"),
    ];

    for (value, expected) in test_cases {
        assert_eq!(expected, format_bytes(value));
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
    if let Some(kb) = bytes.strip_suffix("kiB") {
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
        ("1kiB", 1_024),
        ("1000kiB", 1_000 * 1024),
        ("1MiB", 1024 * 1024),
        ("1000MiB", 1_000 * 1024 * 1024),
        ("1GiB", 1024 * 1024 * 1024),
        ("1000GiB", 1_000 * 1024 * 1024 * 1024),
        ("1TiB", 1024 * 1024 * 1024 * 1024),
        ("1000TiB", 1_000 * 1024 * 1024 * 1024 * 1024),
        ("123B", 123),
        ("12kiB", 12 * 1024),
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
        ("1kiB", 1_024),
        ("1000kiB", 1_000 * 1024),
        ("1MiB", 1024 * 1024),
        ("1000MiB", 1_000 * 1024 * 1024),
        ("1GiB", 1024 * 1024 * 1024),
        ("1000GiB", 1_000 * 1024 * 1024 * 1024),
        ("1TiB", 1024 * 1024 * 1024 * 1024),
        ("1000TiB", 1_000 * 1024 * 1024 * 1024 * 1024),
        ("123B", 123),
        ("12kiB", 12 * 1024),
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
