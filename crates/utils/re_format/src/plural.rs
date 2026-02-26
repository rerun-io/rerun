use std::fmt::Display;

use crate::{UnsignedAbs, format_int, format_uint};

/// Returns either "1 $NOUN" (if `count` is one), otherwise returns `$N $NOUNs`.
pub fn format_plural_s(count: impl num_traits::Unsigned + Display, noun: &'static str) -> String {
    if count.is_one() {
        format!("1 {noun}")
    } else {
        format!("{} {noun}s", format_uint(count))
    }
}

/// Like [`format_plural_s`], but also handles `-1` as singular.
pub fn format_plural_signed_s<Int>(count: Int, noun: &'static str) -> String
where
    Int: num_traits::Signed + Display + PartialOrd + num_traits::Zero + UnsignedAbs,
    Int::Unsigned: Display + num_traits::Unsigned,
{
    if count.abs().is_one() {
        format!("{} {noun}", format_int(count))
    } else {
        format!("{} {noun}s", format_int(count))
    }
}

#[test]
fn test_format_plural_s() {
    assert_eq!(format_plural_s(0_usize, "item"), "0 items");
    assert_eq!(format_plural_s(1_usize, "item"), "1 item");
    assert_eq!(format_plural_s(2_usize, "item"), "2 items");
    assert_eq!(format_plural_s(100_usize, "component"), "100 components");
}

#[test]
fn test_format_plural_signed_s() {
    let minus = crate::MINUS;
    assert_eq!(format_plural_signed_s(0_isize, "frame"), "0 frames");
    assert_eq!(format_plural_signed_s(1_isize, "frame"), "1 frame");
    assert_eq!(
        format_plural_signed_s(-1_isize, "frame"),
        format!("{minus}1 frame")
    );
    assert_eq!(format_plural_signed_s(2_isize, "frame"), "2 frames");
    assert_eq!(
        format_plural_signed_s(-2_isize, "frame"),
        format!("{minus}2 frames")
    );
    assert_eq!(
        format_plural_signed_s(-100_isize, "offset"),
        format!("{minus}100 offsets")
    );
}
