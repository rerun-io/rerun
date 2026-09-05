use super::*;
use crate::vulkan::test_device;

fn access_units(data: &[u8]) -> Vec<Vec<u8>> {
    let starts: Vec<_> = data
        .windows(4)
        .enumerate()
        .filter_map(|(index, bytes)| (bytes == [0, 0, 1, 9]).then_some(index))
        .collect();
    assert!(starts.len() >= 2, "test asset needs access unit delimiters");
    starts
        .iter()
        .enumerate()
        .map(|(index, &start)| {
            data[start..starts.get(index + 1).copied().unwrap_or(data.len())].to_vec()
        })
        .collect()
}

/// Decode and copy submissions use separate timelines on different queues.
/// A decode into a different output image can run while the preceding copy is pending.
#[test]
fn decode_and_copy_use_separate_timelines() {
    let mut core = DecoderCore::new(test_device::shared()).unwrap();
    let units = access_units(include_bytes!("../../tests/assets/ippp.h264"));
    let first_info = core.parse(&units[0]).unwrap().remove(0);
    let first = core.submit_decode(&first_info, &units[0]).unwrap();
    core.submit_copy(&first, |_, _| {}).unwrap();
    let second_info = core.parse(&units[1]).unwrap().remove(0);
    core.submit_decode(&second_info, &units[1]).unwrap();

    test_device::with_state(|state| {
        assert_eq!(state.submissions.len(), 3);
        let copy = &state.submissions[1];
        let decode = &state.submissions[2];
        assert_ne!(copy.queue, decode.queue);
        let first = &state.submissions[0];
        assert_eq!(first.signals.len(), 1);
        assert_eq!(copy.signals.len(), 1);
        assert_eq!(decode.signals.len(), 1);
        assert_ne!(copy.signals[0].0, decode.signals[0].0);
        assert!(copy.waits.contains(&first.signals[0]));
        assert_eq!(decode.waits, first.signals);
        assert_eq!(decode.signals[0].0, first.signals[0].0);
        assert!(decode.signals[0].1 > first.signals[0].1);
    });
}

/// Later decodes complete while an earlier output copy remains pending.
/// Only the copy timeline makes output frames available.
#[test]
fn decode_completion_does_not_complete_pending_copies() {
    let mut decoder = TextureDecoder::new(test_device::shared()).unwrap();
    let units = access_units(include_bytes!("../../tests/assets/ippp.h264"));
    decoder.push_access_unit(&units[0], 0).unwrap();
    decoder.push_access_unit(&units[1], 1).unwrap();
    test_device::with_state(|state| state.complete_submission(2));
    assert_eq!(decoder.core.poll_completed().unwrap(), 0);
    assert_eq!(decoder.pending.len(), 2);
    assert_eq!(decoder.core.in_flight.len(), 2);

    test_device::with_state(|state| state.complete_submission(1));
    assert_eq!(
        decoder.core.poll_completed().unwrap(),
        decoder.pending[0].copy_value
    );
    assert_eq!(decoder.core.in_flight.len(), 1);
}

/// The driver reports a failed decode after its output copy completes.
/// Polling again must not make that failed frame available for presentation.
#[test]
fn failed_decode_output_is_not_available_after_another_poll() {
    let mut decoder = TextureDecoder::new(test_device::shared()).unwrap();
    let units = access_units(include_bytes!("../../tests/assets/ippp.h264"));
    assert!(decoder.push_access_unit(&units[0], 0).unwrap().is_empty());
    assert_eq!(decoder.pending.len(), 1);
    test_device::with_state(|state| {
        state.complete_submission(1);
        state.query_status = -1;
    });

    assert!(matches!(
        decoder.flush(),
        Err(DecodeError::DecodeFailed(-1))
    ));
    if let Ok(completed) = decoder.core.poll_completed() {
        assert!(
            decoder
                .pending
                .iter()
                .all(|frame| frame.copy_value > completed),
            "the failed frame remains ready for `take_completed` after its error was reported"
        );
    }
}

/// Reset waits for GPU work and drops failed frames before decoding the next IDR.
#[test]
fn reset_clears_failed_decode_status() {
    let mut decoder = TextureDecoder::new(test_device::shared()).unwrap();
    let units = access_units(include_bytes!("../../tests/assets/ippp.h264"));
    decoder.push_access_unit(&units[0], 0).unwrap();
    test_device::with_state(|state| {
        state.complete_submission(1);
        state.query_status = -1;
    });
    assert!(matches!(
        decoder.flush(),
        Err(DecodeError::DecodeFailed(-1))
    ));

    decoder.reset();
    assert!(decoder.pending.is_empty());
    assert!(decoder.core.in_flight.is_empty());
    assert!(decoder.core.pending_layer_copies.is_empty());
    assert!(decoder.core.poll_completed().is_ok());
    test_device::with_state(|state| state.query_status = 1);
    let info = decoder.core.parse(&units[0]).unwrap().remove(0);
    let decode = decoder.core.submit_decode(&info, &units[0]).unwrap();
    let copy_value = decoder.core.submit_copy(&decode, |_, _| {}).unwrap();
    test_device::with_state(|state| state.complete_submission(3));
    assert_eq!(decoder.core.poll_completed().unwrap(), copy_value);
    assert!(decoder.core.in_flight.is_empty());
}

/// An SPS declares no reference frames, while successive intra pictures use the sliding window.
/// Every slot selected by the parser must fit in the session created for that SPS.
#[test]
fn zero_declared_references_fit_the_video_session() {
    let mut core = DecoderCore::new(test_device::shared()).unwrap();
    let units = access_units(include_bytes!("../../tests/assets/i_only.h264"));
    // A non-IDR I slice header with `frame_num` 1, sliding-window marking,
    // zero `slice_qp_delta`, and disabled deblocking. The remaining bits stand
    // in for slice data, which the test device does not read.
    let intra = [0, 0, 1, 0x41, 0xb8, 0xaa, 0x80];
    for unit in [units[0].as_slice(), &intra] {
        let info = core.parse(unit).unwrap().remove(0);
        assert!(info.is_intra);
        assert!(info.references.is_empty());
        assert_eq!(core.sps[&info.sps_id].parsed.info.max_num_ref_frames, 0);
        core.submit_decode(&info, unit).unwrap();
    }
    test_device::with_state(|state| {
        assert_eq!(state.session_slots.len(), 1);
        assert_eq!(state.setup_slots.len(), 2);
        let slots = state.session_slots[0];
        assert!(
            state
                .setup_slots
                .iter()
                .all(|&slot| slot >= 0 && (slot as u32) < slots),
            "the parser selected slots {:?}, but the session has only {slots} slots",
            state.setup_slots
        );
    });
}
