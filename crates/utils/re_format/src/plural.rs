use std::fmt::Display;

/// Returns either "1 $NOUN" (if `count` is one), otherwise returns `$N $NOUNs`.
#[expect(clippy::needless_pass_by_value)]
pub fn format_plural_s(count: impl num_traits::Num + Display, noun: &'static str) -> String {
    if count.is_one() {
        format!("1 {noun}")
    } else {
        format!("{count} {noun}s")
    }
}

/// Like [`format_plural_s`], but also handles `-1` as singular.
#[expect(clippy::needless_pass_by_value)]
pub fn format_plural_signed_s(
    count: impl num_traits::Signed + Display,
    noun: &'static str,
) -> String {
    if count.abs().is_one() {
        format!("{count} {noun}")
    } else {
        format!("{count} {noun}s")
    }
}

#[test]
fn test_format_plural_s() {
    assert_eq!(format_plural_s(0, "item"), "0 items");
    assert_eq!(format_plural_s(1, "item"), "1 item");
    assert_eq!(format_plural_s(2, "item"), "2 items");
    assert_eq!(format_plural_s(100, "component"), "100 components");
}

#[test]
fn test_format_plural_signed_s() {
    assert_eq!(format_plural_signed_s(0, "frame"), "0 frames");
    assert_eq!(format_plural_signed_s(1, "frame"), "1 frame");
    assert_eq!(format_plural_signed_s(-1, "frame"), "-1 frame");
    assert_eq!(format_plural_signed_s(2, "frame"), "2 frames");
    assert_eq!(format_plural_signed_s(-2, "frame"), "-2 frames");
    assert_eq!(format_plural_signed_s(-100, "offset"), "-100 offsets");
}
