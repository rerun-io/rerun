use parking_lot::Mutex;
use re_sdk::{RecordingStream, StoreKind};

use crate::{
    CError, CRecordingStream, RR_REC_STREAM_CURRENT_BLUEPRINT, RR_REC_STREAM_CURRENT_RECORDING,
};

#[derive(Default)]
pub struct RecStreams {
    next_id: CRecordingStream,
    streams: ahash::HashMap<CRecordingStream, RecordingStream>,
}

impl RecStreams {
    pub fn insert(&mut self, stream: RecordingStream) -> CRecordingStream {
        let id = self.next_id;
        self.next_id += 1;
        self.streams.insert(id, stream);
        id
    }

    pub fn get(&self, id: CRecordingStream) -> Option<RecordingStream> {
        match id {
            RR_REC_STREAM_CURRENT_RECORDING => RecordingStream::get(StoreKind::Recording, None)
                .or(Some(RecordingStream::disabled())),
            RR_REC_STREAM_CURRENT_BLUEPRINT => RecordingStream::get(StoreKind::Blueprint, None)
                .or(Some(RecordingStream::disabled())),
            _ => self.streams.get(&id).cloned(),
        }
    }

    pub fn remove(&mut self, id: CRecordingStream) -> Option<RecordingStream> {
        match id {
            RR_REC_STREAM_CURRENT_BLUEPRINT | RR_REC_STREAM_CURRENT_RECORDING => None,
            _ => self.streams.remove(&id),
        }
    }
}

/// All recording streams created from C.
pub static RECORDING_STREAMS: std::sync::LazyLock<Mutex<RecStreams>> =
    std::sync::LazyLock::new(Mutex::default);

/// Access a C created recording stream.
#[expect(clippy::result_large_err)]
pub fn recording_stream(stream: CRecordingStream) -> Result<RecordingStream, CError> {
    RECORDING_STREAMS
        .lock()
        .get(stream)
        .ok_or_else(CError::invalid_recording_stream_handle)
}
