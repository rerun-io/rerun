use crate::{Component, EntityPath};
use arrow2::array::Int128Array;
use arrow2_convert::{field::I128, ArrowDeserialize, ArrowField, ArrowSerialize};

// TODO(filip): Convert to use i128

/// Stats about the XLink connection throughput
#[derive(Clone, Copy, Debug, PartialEq, ArrowField, ArrowSerialize, ArrowDeserialize)]
pub struct XlinkStats {
    /// Bytes read from the XLink by the host (PC)
    pub bytes_read: i64,

    /// Bytes written to the XLink by the host (PC)
    pub bytes_written: i64,

    /// Time in s from epoch when the stats were collected
    pub timestamp: f64,
}

impl XlinkStats {
    pub fn entity_path() -> EntityPath {
        "xlink_stats".into()
    }
}

impl Component for XlinkStats {
    #[inline]
    fn name() -> crate::ComponentName {
        "rerun.xlink_stats".into()
    }
}
