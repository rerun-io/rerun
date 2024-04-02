//! Miscellaneous tools to format and parse numbers, durations, etc.
//!
//! TODO(emilk): move some of this numeric formatting into `emath` so we can use it in `egui_plot`.

#[cfg(feature = "arrow")]
pub mod arrow;

mod time;

pub use time::next_grid_tick_magnitude_ns;

// --- Numbers ---

/// The minus character: <https://www.compart.com/en/unicode/U+2212>
///
/// Looks slightly different from the normal hyphen `-`.
const MINUS: char = '−';

/// Pretty format an unsigned integer by using thousands separators for readability.
///
/// The returned value is for human eyes only, and can not be parsed
/// by the normal `usize::from_str` function.
pub fn format_uint<Uint>(number: Uint) -> String
where
    Uint: Copy + num_traits::Unsigned + std::fmt::Display,
{
    add_thousands_separators(&number.to_string())
}

/// Pretty format a signed number by using thousands separators for readability.
///
/// The returned value is for human eyes only, and can not be parsed
/// by the normal `usize::from_str` function.
pub fn format_i64(number: i64) -> String {
    if number < 0 {
        // TODO(rust-num/num-traits#315): generalize this to all signed integers once https://github.com/rust-num/num-traits/issues/315 lands
        format!("{MINUS}{}", format_uint(number.unsigned_abs()))
    } else {
        add_thousands_separators(&number.to_string())
    }
}

/// Add thousands separators to a number, every three steps,
/// counting from the last character.
fn add_thousands_separators(number: &str) -> String {
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
    assert_eq!(format_uint(42_u32), "42");
    assert_eq!(format_uint(999_u32), "999");
    assert_eq!(format_uint(1_000_u32), "1 000");
    assert_eq!(format_uint(123_456_u32), "123 456");
    assert_eq!(format_uint(1_234_567_u32), "1 234 567");
}

/// Options for how to format a floating point number, e.g. an [`f64`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FloatFormatOptions {
    /// Number of decimals to show after the decimal point.
    ///
    /// If not specified, a number will be picked automatically.
    pub num_decimals: Option<usize>,

    pub strip_trailing_zeros: bool,
}

impl Default for FloatFormatOptions {
    fn default() -> Self {
        Self {
            num_decimals: None,
            strip_trailing_zeros: true,
        }
    }
}

impl FloatFormatOptions {
    #[inline]
    pub fn with_decimals(mut self, num_decimals: usize) -> Self {
        self.num_decimals = Some(num_decimals);
        self
    }

    /// The returned value is for human eyes only, and can not be parsed
    /// by the normal `f64::from_str` function.
    pub fn format_f64(&self, value: f64) -> String {
        let Self {
            num_decimals,
            strip_trailing_zeros,
        } = *self;

        if value.is_nan() {
            "NaN".to_owned()
        } else if value < 0.0 {
            format!("{MINUS}{}", self.format_f64(-value))
        } else if value == f64::INFINITY {
            "∞".to_owned()
        } else if value.round() == value {
            // perfect integer
            format_i64(value.round() as i64)
        } else {
            let num_decimals = num_decimals.unwrap_or_else(|| {
                let magnitude = value.abs().log10();
                (3.5 - magnitude).round().max(1.0) as usize
            });
            let mut formatted = format!("{value:.num_decimals$}");

            if strip_trailing_zeros {
                while formatted.ends_with('0') {
                    formatted.pop();
                }
                if formatted.ends_with('.') {
                    formatted.pop();
                }
            }

            if let Some(dot) = formatted.find('.') {
                let integer_part = &formatted[..dot];
                let fractional_part = &formatted[dot + 1..];
                let integer_part = add_thousands_separators(integer_part);
                // For the fractional part we should start counting thousand separators from the _front_, so we reverse:
                let fractional_part = reverse(&add_thousands_separators(&reverse(fractional_part)));
                format!("{integer_part}.{fractional_part}")
            } else {
                add_thousands_separators(&formatted) // it's an integer
            }
        }
    }
}

/// Format a number with a decent number of decimals.
///
/// The returned value is for human eyes only, and can not be parsed
/// by the normal `f64::from_str` function.
pub fn format_f64(value: f64) -> String {
    FloatFormatOptions::default().format_f64(value)
}

fn reverse(s: &str) -> String {
    s.chars().rev().collect()
}

/// Format a number with a decent number of decimals.
///
/// The returned value is for human eyes only, and can not be parsed
/// by the normal `f64::from_str` function.
pub fn format_f32(value: f32) -> String {
    format_f64(value as f64)
}

#[test]
fn test_format_float() {
    assert_eq!(format_f64(f64::NAN), "NaN");
    assert_eq!(format_f64(f64::INFINITY), "∞");
    assert_eq!(format_f64(f64::NEG_INFINITY), "−∞");
    assert_eq!(format_f64(0.0), "0");
    assert_eq!(format_f64(42.0), "42");
    assert_eq!(format_f64(-42.0), "−42");
    assert_eq!(format_f64(-4.20), "−4.2");
    assert_eq!(format_f64(123_456_789.0), "123 456 789");
    assert_eq!(format_f64(123_456_789.123_45), "123 456 789.1");
    assert_eq!(format_f64(0.0000123456789), "0.000 012 35");
    assert_eq!(format_f64(0.123456789), "0.123 5");
    assert_eq!(format_f64(1.23456789), "1.235");
    assert_eq!(format_f64(12.3456789), "12.35");
    assert_eq!(format_f64(123.456789), "123.5");
    assert_eq!(format_f64(1234.56789), "1 234.6");
    assert_eq!(format_f64(12345.6789), "12 345.7");
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
        format!("{MINUS}{}", approximate_large_number(-number))
    } else if number < 1000.0 {
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
        format!("{MINUS}{}", format_bytes(-number_of_bytes))
    } else if number_of_bytes < 10.0_f64.exp2() {
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
    // Note: intentionally case sensitive so that we don't parse `Mb` (Megabit) as `MB` (Megabyte).
    if let Some(rest) = bytes.strip_prefix(MINUS) {
        Some(-parse_bytes_base10(rest)?)
    } else if let Some(kb) = bytes.strip_suffix("kB") {
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
        ("-10B", -10), // hyphen-minus
        ("−10B", -10), // proper minus
    ];
    for (value, expected) in test_cases {
        assert_eq!(Some(expected), parse_bytes_base10(value));
    }
}

pub fn parse_bytes_base2(bytes: &str) -> Option<i64> {
    // Note: intentionally case sensitive so that we don't parse `Mib` (Mebibit) as `MiB` (Mebibyte).
    if let Some(rest) = bytes.strip_prefix(MINUS) {
        Some(-parse_bytes_base2(rest)?)
    } else if let Some(kb) = bytes.strip_suffix("KiB") {
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
        ("-10B", -10), // hyphen-minus
        ("−10B", -10), // proper minus
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
