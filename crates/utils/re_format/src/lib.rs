//! Miscellaneous tools to format and parse numbers, durations, etc.
//!
//! TODO(emilk): move some of this numeric formatting into `emath` so we can use it in `egui_plot`.

mod duration;
mod plural;
pub mod time;

use std::cmp::PartialOrd;
use std::fmt::Display;

pub use self::duration::DurationFormatOptions;
pub use self::plural::{format_plural_s, format_plural_signed_s};

// --- Numbers ---

/// The minus character: <https://www.compart.com/en/unicode/U+2212>
///
/// Looks slightly different from the normal hyphen `-`.
pub const MINUS: char = '−';

/// A thin space, used for thousands separators, like `1 234`:
///
/// <https://en.wikipedia.org/wiki/Thin_space>
pub const THIN_SPACE: char = '\u{2009}';

/// Prepare a string containing a number for parsing
pub fn strip_whitespace_and_normalize(text: &str) -> String {
    text.chars()
        // Ignore whitespace (trailing, leading, and thousands separators):
        .filter(|c| !c.is_whitespace())
        // Replace special minus character with normal minus (hyphen):
        .map(|c| if c == MINUS { '-' } else { c })
        .collect()
}

// TODO(rust-num/num-traits#315): waiting for https://github.com/rust-num/num-traits/issues/315 to land
pub trait UnsignedAbs {
    /// An unsigned type which is large enough to hold the absolute value of `Self`.
    type Unsigned;

    /// Computes the absolute value of `self` without any wrapping or panicking.
    fn unsigned_abs(self) -> Self::Unsigned;
}

impl UnsignedAbs for i8 {
    type Unsigned = u8;

    #[inline]
    fn unsigned_abs(self) -> Self::Unsigned {
        self.unsigned_abs()
    }
}

impl UnsignedAbs for i16 {
    type Unsigned = u16;

    #[inline]
    fn unsigned_abs(self) -> Self::Unsigned {
        self.unsigned_abs()
    }
}

impl UnsignedAbs for i32 {
    type Unsigned = u32;

    #[inline]
    fn unsigned_abs(self) -> Self::Unsigned {
        self.unsigned_abs()
    }
}

impl UnsignedAbs for i64 {
    type Unsigned = u64;

    #[inline]
    fn unsigned_abs(self) -> Self::Unsigned {
        self.unsigned_abs()
    }
}

impl UnsignedAbs for i128 {
    type Unsigned = u128;

    #[inline]
    fn unsigned_abs(self) -> Self::Unsigned {
        self.unsigned_abs()
    }
}

impl UnsignedAbs for isize {
    type Unsigned = usize;

    #[inline]
    fn unsigned_abs(self) -> Self::Unsigned {
        self.unsigned_abs()
    }
}

/// Pretty format a signed number by using thousands separators for readability.
///
/// The returned value is for human eyes only, and can not be parsed
/// by the normal `usize::from_str` function.
pub fn format_int<Int>(number: Int) -> String
where
    Int: Display + PartialOrd + num_traits::Zero + UnsignedAbs,
    Int::Unsigned: Display + num_traits::Unsigned,
{
    if number < Int::zero() {
        format!("{MINUS}{}", format_uint(number.unsigned_abs()))
    } else {
        add_thousands_separators(&number.to_string())
    }
}

/// Pretty format an unsigned integer by using thousands separators for readability.
///
/// The returned value is for human eyes only, and can not be parsed
/// by the normal `usize::from_str` function.
#[expect(clippy::needless_pass_by_value)]
pub fn format_uint<Uint>(number: Uint) -> String
where
    Uint: Display + num_traits::Unsigned,
{
    add_thousands_separators(&number.to_string())
}

/// Add thousands separators to a number, every three steps,
/// counting from the last character.
fn add_thousands_separators(number: &str) -> String {
    let mut chars = number.chars().rev().peekable();

    let mut result = vec![];
    while chars.peek().is_some() {
        if !result.is_empty() {
            // thousands-deliminator:
            result.push(THIN_SPACE);
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
fn test_format_uint() {
    assert_eq!(format_uint(42_u32), "42");
    assert_eq!(format_uint(999_u32), "999");
    assert_eq!(format_uint(1_000_u32), "1 000");
    assert_eq!(format_uint(123_456_u32), "123 456");
    assert_eq!(format_uint(1_234_567_u32), "1 234 567");
}

/// Options for how to format a floating point number, e.g. an [`f64`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FloatFormatOptions {
    /// Always show the sign, even if it is positive (`+`).
    pub always_sign: bool,

    /// Maximum digits of precision to use.
    ///
    /// This includes both the integer part and the fractional part.
    pub precision: usize,

    /// Max number of decimals to show after the decimal point.
    ///
    /// If not specified, [`Self::precision`] is used instead.
    pub num_decimals: Option<usize>,

    pub strip_trailing_zeros: bool,

    /// Only add thousands separators to decimals if there are at least this many decimals.
    pub min_decimals_for_thousands_separators: usize,
}

impl FloatFormatOptions {
    /// Default options for formatting an [`half::f16`].
    #[expect(non_upper_case_globals)]
    pub const DEFAULT_f16: Self = Self {
        always_sign: false,
        precision: 5,
        num_decimals: None,
        strip_trailing_zeros: true,
        min_decimals_for_thousands_separators: 6,
    };

    /// Default options for formatting an [`f32`].
    #[expect(non_upper_case_globals)]
    pub const DEFAULT_f32: Self = Self {
        always_sign: false,
        precision: 7,
        num_decimals: None,
        strip_trailing_zeros: true,
        min_decimals_for_thousands_separators: 6,
    };

    /// Default options for formatting an [`f64`].
    #[expect(non_upper_case_globals)]
    pub const DEFAULT_f64: Self = Self {
        always_sign: false,
        precision: 15,
        num_decimals: None,
        strip_trailing_zeros: true,
        min_decimals_for_thousands_separators: 6,
    };

    /// Always show the sign, even if it is positive (`+`).
    #[inline]
    pub fn with_always_sign(mut self, always_sign: bool) -> Self {
        self.always_sign = always_sign;
        self
    }

    /// Show at most this many digits of precision,
    /// including both the integer part and the fractional part.
    #[inline]
    pub fn with_precision(mut self, precision: usize) -> Self {
        self.precision = precision;
        self
    }

    /// Max number of decimals to show after the decimal point.
    ///
    /// If not specified, [`Self::precision`] is used instead.
    #[inline]
    pub fn with_decimals(mut self, num_decimals: usize) -> Self {
        self.num_decimals = Some(num_decimals);
        self
    }

    /// Strip trailing zeros from decimal expansion?
    #[inline]
    pub fn with_strip_trailing_zeros(mut self, strip_trailing_zeros: bool) -> Self {
        self.strip_trailing_zeros = strip_trailing_zeros;
        self
    }

    /// The returned value is for human eyes only, and can not be parsed
    /// by the normal `f64::from_str` function.
    pub fn format(&self, value: impl Into<f64>) -> String {
        self.format_f64(value.into())
    }

    fn format_f64(&self, mut value: f64) -> String {
        fn reverse(s: &str) -> String {
            s.chars().rev().collect()
        }

        let Self {
            always_sign,
            precision,
            num_decimals,
            strip_trailing_zeros,
            min_decimals_for_thousands_separators,
        } = *self;

        if value.is_nan() {
            return "NaN".to_owned();
        }

        let sign = if value < 0.0 {
            value = -value;
            "−" // NOTE: the minus character: <https://www.compart.com/en/unicode/U+2212>
        } else if always_sign {
            "+"
        } else {
            ""
        };

        let abs_string = if value == f64::INFINITY {
            "∞".to_owned()
        } else {
            let magnitude = value.log10();
            let max_decimals = precision as f64 - magnitude.max(0.0);

            if max_decimals < 0.0 {
                // A very large number (more digits than we have precision),
                // so use scientific notation.
                // TODO(emilk): nice formatting of scientific notation with thousands separators
                format!("{:.*e}", precision.saturating_sub(1), value)
            } else {
                let max_decimals = max_decimals as usize;

                let num_decimals = if let Some(num_decimals) = num_decimals {
                    num_decimals.min(max_decimals)
                } else {
                    max_decimals
                };

                let mut formatted = format!("{value:.num_decimals$}");

                if strip_trailing_zeros && formatted.contains('.') {
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
                    // let fractional_part = &fractional_part[..num_decimals.min(fractional_part.len())];

                    let integer_part = add_thousands_separators(integer_part);

                    if fractional_part.len() < min_decimals_for_thousands_separators {
                        format!("{integer_part}.{fractional_part}")
                    } else {
                        // For the fractional part we should start counting thousand separators from the _front_, so we reverse:
                        let fractional_part =
                            reverse(&add_thousands_separators(&reverse(fractional_part)));
                        format!("{integer_part}.{fractional_part}")
                    }
                } else {
                    add_thousands_separators(&formatted) // it's an integer
                }
            }
        };

        format!("{sign}{abs_string}")
    }
}

/// Format a number with about 15 decimals of precision.
///
/// The returned value is for human eyes only, and can not be parsed
/// by the normal `f64::from_str` function.
pub fn format_f64(value: f64) -> String {
    FloatFormatOptions::DEFAULT_f64.format(value)
}

/// Format a number with about 7 decimals of precision.
///
/// The returned value is for human eyes only, and can not be parsed
/// by the normal `f64::from_str` function.
pub fn format_f32(value: f32) -> String {
    FloatFormatOptions::DEFAULT_f32.format(value)
}

/// Format a number with about 5 decimals of precision.
///
/// The returned value is for human eyes only, and can not be parsed
/// by the normal `f64::from_str` function.
pub fn format_f16(value: half::f16) -> String {
    FloatFormatOptions::DEFAULT_f16.format(value)
}

/// Format a latitude or longitude value.
///
/// For human eyes only.
pub fn format_lat_lon(value: f64) -> String {
    format!(
        "{}°",
        FloatFormatOptions {
            always_sign: true,
            precision: 10,
            num_decimals: Some(6),
            strip_trailing_zeros: false,
            min_decimals_for_thousands_separators: 10,
        }
        .format_f64(value)
    )
}

#[test]
fn test_format_f32() {
    let cases = [
        (f32::NAN, "NaN"),
        (f32::INFINITY, "∞"),
        (f32::NEG_INFINITY, "−∞"),
        (0.0, "0"),
        (42.0, "42"),
        (10_000.0, "10 000"),
        (1_000_000.0, "1 000 000"),
        (10_000_000.0, "10 000 000"),
        (11_000_000.0, "1.100000e7"),
        (-42.0, "−42"),
        (-4.20, "−4.2"),
        (123_456.78, "123 456.8"),
        (78.4321, "78.4321"), // min_decimals_for_thousands_separators
        (-std::f32::consts::PI, "−3.141 593"),
        (-std::f32::consts::PI * 1e6, "−3 141 593"),
        (-std::f32::consts::PI * 1e20, "−3.141593e20"), // We switch to scientific notation to not show false precision
    ];
    for (value, expected) in cases {
        let got = format_f32(value);
        assert!(
            got == expected,
            "Expected to format {value} as '{expected}', but got '{got}'"
        );
    }
}

#[test]
fn test_format_f64() {
    let cases = [
        (f64::NAN, "NaN"),
        (f64::INFINITY, "∞"),
        (f64::NEG_INFINITY, "−∞"),
        (0.0, "0"),
        (42.0, "42"),
        (-42.0, "−42"),
        (-4.20, "−4.2"),
        (123_456_789.0, "123 456 789"),
        (123_456_789.123_45, "123 456 789.12345"), // min_decimals_for_thousands_separators
        (0.0000123456789, "0.000 012 345 678 9"),
        (0.123456789, "0.123 456 789"),
        (1.23456789, "1.234 567 89"),
        (12.3456789, "12.345 678 9"),
        (123.456789, "123.456 789"),
        (1234.56789, "1 234.56789"), // min_decimals_for_thousands_separators
        (12345.6789, "12 345.6789"), // min_decimals_for_thousands_separators
        (78.4321, "78.4321"),        // min_decimals_for_thousands_separators
        (-std::f64::consts::PI, "−3.141 592 653 589 79"),
        (-std::f64::consts::PI * 1e6, "−3 141 592.653 589 79"),
        (-std::f64::consts::PI * 1e20, "−3.14159265358979e20"), // We switch to scientific notation to not show false precision
    ];
    for (value, expected) in cases {
        let got = format_f64(value);
        assert!(
            got == expected,
            "Expected to format {value} as '{expected}', but got '{got}'"
        );
    }
}

#[test]
fn test_format_f16() {
    use half::f16;

    let cases = [
        (f16::from_f32(f32::NAN), "NaN"),
        (f16::INFINITY, "∞"),
        (f16::NEG_INFINITY, "−∞"),
        (f16::ZERO, "0"),
        (f16::from_f32(42.0), "42"),
        (f16::from_f32(-42.0), "−42"),
        (f16::from_f32(-4.20), "−4.1992"), // f16 precision limitation
        (f16::from_f32(12_345.0), "12 344"), // f16 precision limitation
        (f16::PI, "3.1406"),               // f16 precision limitation
    ];
    for (value, expected) in cases {
        let got = format_f16(value);
        assert_eq!(
            got, expected,
            "Expected to format {value} as '{expected}', but got '{got}'"
        );
    }
}

#[test]
fn test_format_f64_custom() {
    let cases = [(
        FloatFormatOptions::DEFAULT_f64.with_decimals(2),
        123.456789,
        "123.46",
    )];
    for (options, value, expected) in cases {
        let got = options.format(value);
        assert!(
            got == expected,
            "Expected to format {value} as '{expected}', but got '{got}'. Options: {options:#?}"
        );
    }
}

/// Parses a number, ignoring whitespace (e.g. thousand separators),
/// and treating the special minus character `MINUS` (−) as a minus sign.
pub fn parse_f64(text: &str) -> Option<f64> {
    let text = strip_whitespace_and_normalize(text);
    text.parse().ok()
}

/// Parses a number, ignoring whitespace (e.g. thousand separators),
/// and treating the special minus character `MINUS` (−) as a minus sign.
pub fn parse_i64(text: &str) -> Option<i64> {
    let text = strip_whitespace_and_normalize(text);
    text.parse().ok()
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
/// Prefer to use [`format_uint`], which outputs an exact string,
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
    } else if number_of_bytes == 0.0 {
        "0 B".to_owned()
    } else if number_of_bytes < 1.0 {
        format!("{number_of_bytes} B")
    } else if number_of_bytes < 20.0 {
        let is_integer = number_of_bytes.round() == number_of_bytes;
        if is_integer {
            format!("{number_of_bytes:.0} B")
        } else {
            format!("{number_of_bytes:.1} B")
        }
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
        (0.0, "0 B"),
        (0.25, "0.25 B"),
        (1.51, "1.5 B"),
        (11.0, "11 B"),
        (12.5, "12.5 B"),
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
    let bytes = strip_whitespace_and_normalize(bytes);

    if bytes == "0" {
        return Some(0);
    }

    // Note: intentionally case sensitive so that we don't parse `Mb` (Megabit) as `MB` (Megabyte).
    if let Some(rest) = bytes.strip_prefix(MINUS) {
        Some(-parse_bytes_base10(rest)?)
    } else if let Some(kb) = bytes.strip_suffix("kB") {
        Some((kb.parse::<f64>().ok()? * 1e3) as _)
    } else if let Some(mb) = bytes.strip_suffix("MB") {
        Some((mb.parse::<f64>().ok()? * 1e6) as _)
    } else if let Some(gb) = bytes.strip_suffix("GB") {
        Some((gb.parse::<f64>().ok()? * 1e9) as _)
    } else if let Some(tb) = bytes.strip_suffix("TB") {
        Some((tb.parse::<f64>().ok()? * 1e12) as _)
    } else if let Some(b) = bytes.strip_suffix('B') {
        Some(b.parse::<i64>().ok()?)
    } else {
        None
    }
}

#[test]
fn test_parse_bytes_base10() {
    let test_cases = [
        ("0", 0), // Zero requires no unit
        ("-1B", -1),
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
    let bytes = strip_whitespace_and_normalize(bytes);

    if bytes == "0" {
        return Some(0);
    }

    // Note: intentionally case sensitive so that we don't parse `Mib` (Mebibit) as `MiB` (Mebibyte).
    if let Some(rest) = bytes.strip_prefix(MINUS) {
        Some(-parse_bytes_base2(rest)?)
    } else if let Some(kb) = bytes.strip_suffix("KiB") {
        Some((kb.parse::<f64>().ok()? * 1024.0) as _)
    } else if let Some(mb) = bytes.strip_suffix("MiB") {
        Some((mb.parse::<f64>().ok()? * 1024.0 * 1024.0) as _)
    } else if let Some(gb) = bytes.strip_suffix("GiB") {
        Some((gb.parse::<f64>().ok()? * 1024.0 * 1024.0 * 1024.0) as _)
    } else if let Some(tb) = bytes.strip_suffix("TiB") {
        Some((tb.parse::<f64>().ok()? * 1024.0 * 1024.0 * 1024.0 * 1024.0) as _)
    } else if let Some(b) = bytes.strip_suffix('B') {
        Some(b.parse::<i64>().ok()?)
    } else {
        None
    }
}

#[test]
fn test_parse_bytes_base2() {
    let test_cases = [
        ("0", 0), // Zero requires no unit
        ("-1B", -1),
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
        ("0", 0), // Zero requires no unit
        ("-1B", -1),
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

/// Remove the custom formatting
///
/// Removes the thin spaces and the special minus character. Useful when copying text.
pub fn remove_number_formatting(s: &str) -> String {
    s.chars()
        .filter_map(|c| {
            if c == MINUS {
                Some('-')
            } else if c == THIN_SPACE {
                None
            } else {
                Some(c)
            }
        })
        .collect()
}

#[test]
fn test_remove_number_formatting() {
    assert_eq!(
        remove_number_formatting(&format_f32(-123_456.78)),
        "-123456.8"
    );
    assert_eq!(
        remove_number_formatting(&format_f64(-123_456.78)),
        "-123456.78"
    );
    assert_eq!(
        remove_number_formatting(&format_int(-123_456_789_i32)),
        "-123456789"
    );
    assert_eq!(
        remove_number_formatting(&format_uint(123_456_789_u32)),
        "123456789"
    );
}
