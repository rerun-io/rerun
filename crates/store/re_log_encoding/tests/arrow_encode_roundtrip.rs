use re_chunk::{Chunk, RowId, TimePoint, Timeline};
use re_log_encoding::{DecoderApp, Encoder};
use re_log_types::{LogMsg, StoreId};
use re_sdk_types::archetypes::Points3D;
use similar_asserts::assert_eq;

fn no_radii() -> impl Iterator<Item = f32> {
    std::iter::empty()
}

#[test]
fn encode_roundtrip() {
    fn timepoint(time: i64) -> TimePoint {
        TimePoint::default().with(Timeline::new_sequence("my_index"), time)
    }

    let chunk = Chunk::builder("points")
        .with_archetype(
            RowId::new(),
            timepoint(1),
            &Points3D::new([[1., 2., 3.], [4., 5., 6.], [7., 8., 9.]]).with_radii(no_radii()),
        )
        .with_archetype(
            RowId::new(),
            timepoint(1),
            &Points3D::new([[10., 11., 12.]]).with_colors([[255, 0, 0]]),
        )
        .build()
        .unwrap();

    let record_batch = chunk.to_record_batch().unwrap();
    assert_eq!(Chunk::from_record_batch(&record_batch).unwrap(), chunk);

    let arrow_msg = chunk.to_arrow_msg().unwrap();
    assert_eq!(Chunk::from_arrow_msg(&arrow_msg).unwrap(), chunk);

    let store_id = StoreId::empty_recording();
    let messages = [LogMsg::ArrowMsg(store_id, arrow_msg)];

    let encoded = Encoder::encode(messages.iter().cloned().map(Ok)).unwrap();
    let decoded: Vec<_> = DecoderApp::decode_lazy(encoded.as_slice())
        .map(Result::unwrap)
        .collect();
    similar_asserts::assert_eq!(decoded, messages, "Failed to roundtrip chunk");
}
