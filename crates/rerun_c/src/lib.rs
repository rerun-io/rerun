//! The Rerun C SDK.
//!
//! The functions here must match `rerun.h`.
// TODO(emilk): error handling

#![crate_type = "staticlib"]
#![allow(clippy::missing_safety_doc, clippy::undocumented_unsafe_blocks)] // Too much unsafe

use std::ffi::{c_char, CStr, CString};

use once_cell::sync::Lazy;
use parking_lot::Mutex;

use re_sdk::{
    external::re_log_types::{StoreInfo, StoreSource},
    sink::TcpSink,
    time::Time,
    ApplicationId, RecordingStream, StoreId, StoreKind,
};

// ----------------------------------------------------------------------------
// Types:

type CRecStreamId = u32;

#[repr(u32)]
#[derive(Debug)]
pub enum CStoreKind {
    /// A recording of user-data.
    Recording = 1,

    /// Data associated with the blueprint state.
    Blueprint = 2,
}

/// Simple C version of [`StroeInfo`]
#[repr(C)]
#[derive(Debug)]
pub struct CStoreInfo {
    /// The user-chosen name of the application doing the logging.
    pub application_id: *const c_char,

    pub store_kind: u32, // CStoreKind
}

#[derive(Default)]
pub struct RecStreams {
    next_id: CRecStreamId,
    streams: ahash::HashMap<CRecStreamId, RecordingStream>,
}

impl RecStreams {
    fn insert(&mut self, stream: RecordingStream) -> CRecStreamId {
        let id = self.next_id;
        self.next_id += 1;
        self.streams.insert(id, stream);
        id
    }
}

// ----------------------------------------------------------------------------
// Global data:

/// All recording streams created from C.
static RECORDING_STREAMS: Lazy<Mutex<RecStreams>> = Lazy::new(Mutex::default);

// ----------------------------------------------------------------------------
// Public functions:

// SAFETY: the unsafety comes from #[no_mangle], because we can declare multiple
// functions with the same symbol names, and the linker behavior in this case i undefined.
#[allow(unsafe_code)]
#[no_mangle]
pub extern "C" fn rerun_version_string() -> *const c_char {
    static VERSION: Lazy<CString> =
        Lazy::new(|| CString::new(re_sdk::build_info().to_string()).unwrap()); // unwrap: there won't be any NUL bytes in the string

    VERSION.as_ptr()
}

#[allow(unsafe_code)]
#[no_mangle]
pub unsafe extern "C" fn rerun_rec_stream_new(
    cstore_info: *const CStoreInfo,
    tcp_addr: *const c_char,
) -> CRecStreamId {
    initialize_logging();

    let cstore_info = unsafe { &*cstore_info };

    let CStoreInfo {
        application_id,
        store_kind,
    } = *cstore_info;
    let application_id = unsafe { CStr::from_ptr(application_id) };

    let application_id =
        ApplicationId::from(application_id.to_str().expect("invalid application_id"));

    let store_kind = match store_kind {
        1 => StoreKind::Recording,
        2 => StoreKind::Blueprint,
        _ => panic!("invalid store_kind: expected 1 or 2, got {store_kind}"),
    };

    let store_info = StoreInfo {
        application_id,
        store_id: StoreId::random(store_kind),
        is_official_example: false,
        started: Time::now(),
        store_source: StoreSource::CSdk,
        store_kind,
    };

    let batcher_config = Default::default();

    let tcp_addr = unsafe { CStr::from_ptr(tcp_addr) };
    let tcp_addr = tcp_addr
        .to_str()
        .expect("invalid tcp_addr string")
        .parse()
        .expect("invalid tcp_addr");
    let sink = Box::new(TcpSink::new(tcp_addr));

    let rec_stream = RecordingStream::new(store_info, batcher_config, sink).unwrap();

    RECORDING_STREAMS.lock().insert(rec_stream)
}

#[allow(unsafe_code)]
#[no_mangle]
pub unsafe extern "C" fn rerun_rec_stream_free(id: CRecStreamId) {
    let mut lock = RECORDING_STREAMS.lock();
    if let Some(sink) = lock.streams.remove(&id) {
        sink.disconnect();
    }
}

// ----------------------------------------------------------------------------
// Helper functions:

fn initialize_logging() {
    use std::sync::Once;
    static START: Once = Once::new();
    START.call_once(|| {
        re_log::setup_native_logging();
    });
}

// ----------------------------------------------------------------------------
