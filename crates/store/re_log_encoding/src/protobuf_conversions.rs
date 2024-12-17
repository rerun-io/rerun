impl From<re_protos::log_msg::v0::Compression> for crate::Compression {
    fn from(value: re_protos::log_msg::v0::Compression) -> Self {
        match value {
            re_protos::log_msg::v0::Compression::None => Self::Off,
            re_protos::log_msg::v0::Compression::Lz4 => Self::LZ4,
        }
    }
}

impl From<crate::Compression> for re_protos::log_msg::v0::Compression {
    fn from(value: crate::Compression) -> Self {
        match value {
            crate::Compression::Off => Self::None,
            crate::Compression::LZ4 => Self::Lz4,
        }
    }
}
