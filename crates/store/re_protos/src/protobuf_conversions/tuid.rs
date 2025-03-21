impl From<crate::common::v1alpha1::Tuid> for re_tuid::Tuid {
    fn from(value: crate::common::v1alpha1::Tuid) -> Self {
        Self::from_nanos_and_inc(value.time_ns, value.inc)
    }
}

impl From<re_tuid::Tuid> for crate::common::v1alpha1::Tuid {
    fn from(value: re_tuid::Tuid) -> Self {
        Self {
            time_ns: value.nanos_since_epoch(),
            inc: value.inc(),
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_tuid_conversion() {
        let tuid = re_tuid::Tuid::new();
        let proto_tuid: crate::common::v1alpha1::Tuid = tuid.into();
        let tuid2: re_tuid::Tuid = proto_tuid.into();
        assert_eq!(tuid, tuid2);
    }
}
