use crate::{
    archetypes::EncodedDepthImage,
    components::{Blob, ImageFormat, MediaType},
    datatypes::ChannelDatatype,
};

impl EncodedDepthImage {
    /// Construct from encoded bytes with explicit format metadata.
    pub fn from_encoded_bytes(bytes: impl Into<Vec<u8>>, format: impl Into<ImageFormat>) -> Self {
        Self::new(Blob::from(bytes.into()), format)
    }

    /// Convenience helper for RVL-compressed depth streams.
    pub fn from_rvl_bytes(
        bytes: impl Into<Vec<u8>>,
        width: u32,
        height: u32,
        datatype: ChannelDatatype,
    ) -> Self {
        let format = ImageFormat::depth([width, height], datatype);
        Self::from_encoded_bytes(bytes, format).with_media_type(MediaType::rvl())
    }
}
