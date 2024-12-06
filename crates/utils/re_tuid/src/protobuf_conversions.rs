impl From<re_protos::common::v0::Tuid> for crate::Tuid {
    fn from(value: re_protos::common::v0::Tuid) -> Self {
        Self {
            time_ns: value.time_ns,
            inc: value.inc,
        }
    }
}

impl From<crate::Tuid> for re_protos::common::v0::Tuid {
    fn from(value: crate::Tuid) -> Self {
        Self {
            time_ns: value.time_ns,
            inc: value.inc,
        }
    }
}
