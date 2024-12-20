pub mod decoder;
pub mod encoder;

pub use decoder::decode;
pub use encoder::encode;

#[cfg(test)]
mod tests {
    use crate::{
        codec::wire::{decode, encode},
        codec::CodecError,
    };
    use re_chunk::{Chunk, RowId};
    use re_log_types::{example_components::MyPoint, Timeline};
    use re_protos::common::v0::EncoderVersion;

    fn get_test_chunk() -> Chunk {
        let row_id1 = RowId::new();
        let row_id2 = RowId::new();

        let timepoint1 = [
            (Timeline::log_time(), 100),
            (Timeline::new_sequence("frame"), 1),
        ];
        let timepoint2 = [
            (Timeline::log_time(), 104),
            (Timeline::new_sequence("frame"), 1),
        ];

        let points1 = &[MyPoint::new(1.0, 1.0)];
        let points2 = &[MyPoint::new(2.0, 2.0)];

        Chunk::builder("mypoints".into())
            .with_component_batches(row_id1, timepoint1, [points1 as _])
            .with_component_batches(row_id2, timepoint2, [points2 as _])
            .build()
            .unwrap()
    }

    #[test]
    fn test_invalid_data() {
        let data = vec![2, 3, 4];
        let decoded = decode(EncoderVersion::V0, &data);

        assert!(matches!(
            decoded.err().unwrap(),
            CodecError::ArrowSerialization(_)
        ));
    }

    #[test]
    fn test_v0_codec() {
        let expected_chunk = get_test_chunk();

        let encoded = encode(
            EncoderVersion::V0,
            &expected_chunk.clone().to_transport().unwrap(),
        )
        .unwrap();
        let decoded = decode(EncoderVersion::V0, &encoded).unwrap();
        let decoded_chunk = Chunk::from_transport(&decoded).unwrap();

        assert_eq!(expected_chunk, decoded_chunk);
    }
}
