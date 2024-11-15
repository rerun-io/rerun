mod generated;
pub use self::generated::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn r0_is_empty_o0_is_empty() {
        let r0 = std::iter::empty::<u32>();
        let o0 = (0..).map(|n| n.to_string());

        let expected: Vec<(u32, String)> = vec![];
        let got = clamped_zip_1x1(r0, o0, String::new).collect::<Vec<_>>();

        similar_asserts::assert_eq!(expected, got);
    }

    #[test]
    fn r0_and_o0_are_matched() {
        let r0 = 0..20u32;
        let o0 = (0..20).map(|n| n.to_string());

        let expected: Vec<(u32, String)> = (0..20u32).map(|n| (n, n.to_string())).collect();
        let got = clamped_zip_1x1(r0, o0, String::new).collect::<Vec<_>>();

        similar_asserts::assert_eq!(expected, got);
    }

    #[test]
    fn r0_is_shorter() {
        let r0 = 0..10u32;
        let o0 = (0..20).map(|n| n.to_string());

        let expected: Vec<(u32, String)> = (0..10u32).map(|n| (n, n.to_string())).collect();
        let got = clamped_zip_1x1(r0, o0, String::new).collect::<Vec<_>>();

        similar_asserts::assert_eq!(expected, got);
    }

    #[test]
    fn r0_is_longer() {
        let r0 = 0..30u32;
        let o0 = (0..20).map(|n| n.to_string());

        let expected: Vec<(u32, String)> = (0..30u32)
            .map(|n| (n, u32::min(n, 19).to_string()))
            .collect();
        let got = clamped_zip_1x1(r0, o0, String::new).collect::<Vec<_>>();

        similar_asserts::assert_eq!(expected, got);
    }

    #[test]
    fn r0_is_longer_and_o0_is_empty() {
        let r0 = 0..10u32;
        let o0 = std::iter::empty();

        let expected: Vec<(u32, String)> = (0..10u32).map(|n| (n, "hey".to_owned())).collect();
        let got = clamped_zip_1x1(r0, o0, || "hey".to_owned()).collect::<Vec<_>>();

        similar_asserts::assert_eq!(expected, got);
    }
}
