mod generated;
pub use self::generated::*;

#[cfg(test)]
mod tests {
    use itertools::Itertools as _;
    use re_chunk::RowId;
    use re_log_types::TimeInt;

    use super::*;

    #[test]
    fn overview_1x1() {
        let t9 = TimeInt::new_temporal(9);
        let t10 = TimeInt::new_temporal(10);
        let t11 = TimeInt::new_temporal(11);
        let t12 = TimeInt::new_temporal(12);
        let t13 = TimeInt::new_temporal(13);
        let t14 = TimeInt::new_temporal(14);

        let p0: Vec<((TimeInt, RowId), u32)> = vec![
            ((t9, RowId::ZERO), 90), //
            //
            ((t10, RowId::ZERO), 100), //
            //
            ((t13, RowId::ZERO.incremented_by(0)), 130), //
            ((t13, RowId::ZERO.incremented_by(0)), 130), //
            ((t13, RowId::ZERO.incremented_by(0)), 130), //
            ((t13, RowId::ZERO.incremented_by(1)), 131), //
            ((t13, RowId::ZERO.incremented_by(2)), 132), //
            ((t13, RowId::ZERO.incremented_by(5)), 135), //
            //
            ((t14, RowId::ZERO), 140), //
        ];

        let c0: Vec<((TimeInt, RowId), &'static str)> = vec![
            ((t10, RowId::ZERO.incremented_by(1)), "101"), //
            ((t10, RowId::ZERO.incremented_by(2)), "102"), //
            ((t10, RowId::ZERO.incremented_by(3)), "103"), //
            //
            ((t11, RowId::ZERO), "110"), //
            //
            ((t12, RowId::ZERO), "120"), //
            //
            ((t13, RowId::ZERO.incremented_by(1)), "131"), //
            ((t13, RowId::ZERO.incremented_by(2)), "132"), //
            ((t13, RowId::ZERO.incremented_by(4)), "134"), //
            ((t13, RowId::ZERO.incremented_by(6)), "136"), //
        ];

        let expected: Vec<((TimeInt, RowId), u32, Option<&'static str>)> = vec![
            ((t9, RowId::ZERO), 90, None), //
            //
            ((t10, RowId::ZERO), 100, None), //
            //
            ((t13, RowId::ZERO.incremented_by(0)), 130, Some("120")), //
            ((t13, RowId::ZERO.incremented_by(0)), 130, Some("120")), //
            ((t13, RowId::ZERO.incremented_by(0)), 130, Some("120")), //
            ((t13, RowId::ZERO.incremented_by(1)), 131, Some("131")), //
            ((t13, RowId::ZERO.incremented_by(2)), 132, Some("132")), //
            ((t13, RowId::ZERO.incremented_by(5)), 135, Some("134")), //
            //
            ((t14, RowId::ZERO), 140, Some("136")), //
        ];
        let got = range_zip_1x1(p0, c0).collect_vec();

        similar_asserts::assert_eq!(expected, got);
    }
}
