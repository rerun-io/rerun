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
    external::re_log_types::{self, StoreInfo, StoreSource},
    log::{DataCell, DataRow},
    sink::TcpSink,
    time::Time,
    ApplicationId, ComponentName, EntityPath, RecordingStream, StoreId, StoreKind,
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

#[repr(C)]
pub struct CDataCell {
    pub component_name: *const c_char,

    /// Lenght of [`bytes`].
    pub num_bytes: u64,

    /// Data in the Arrow IPC encapsulated message format.
    pub bytes: *const u8,
}

#[repr(C)]
pub struct CDataRow {
    pub entity_path: *const c_char,
    pub num_instances: u32,
    pub num_data_cells: u32,
    pub data_cells: *const CDataCell,
}

// ----------------------------------------------------------------------------
// Global data:

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

    assert!(!tcp_addr.is_null());
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
pub extern "C" fn rerun_rec_stream_free(id: CRecStreamId) {
    let mut lock = RECORDING_STREAMS.lock();
    if let Some(sink) = lock.streams.remove(&id) {
        sink.disconnect();
    }
}

#[allow(unsafe_code)]
#[no_mangle]
pub unsafe extern "C" fn rerun_log(stream: CRecStreamId, data_row: *const CDataRow) {
    let lock = RECORDING_STREAMS.lock();
    let Some(stream) = lock.streams.get(&stream) else {
        return;
    };

    assert!(!data_row.is_null());
    let data_row = unsafe { &*data_row };

    let CDataRow {
        entity_path,
        num_instances,
        num_data_cells,
        data_cells,
    } = *data_row;

    let entity_path = unsafe { CStr::from_ptr(entity_path) };
    let entity_path =
        EntityPath::from(re_log_types::parse_entity_path(entity_path.to_str().unwrap()).unwrap());

    re_log::debug!(
        "rerun_log {entity_path:?}, num_instances: {num_instances}, num_data_cells: {num_data_cells}",
    );

    let cells = (0..num_data_cells)
        .map(|i| {
            let data_cell: &CDataCell = unsafe { &*data_cells.wrapping_add(i as _) };
            let CDataCell {
                component_name,
                num_bytes,
                bytes,
            } = *data_cell;

            let component_name = unsafe { CStr::from_ptr(component_name) };
            let component_name = ComponentName::from(component_name.to_str().unwrap());

            let bytes = unsafe { std::slice::from_raw_parts(bytes, num_bytes as usize) };

            let array = parse_arrow_ipc_encapsulated_message(bytes).unwrap();

            DataCell::try_from_arrow(component_name, array).unwrap()
        })
        .collect();

    let data_row = DataRow {
        row_id: re_sdk::log::RowId::random(),
        timepoint: Default::default(), // we use the one in the recording stream for now
        entity_path,
        num_instances,
        cells: re_log_types::DataCellRow(cells),
    };

    let inject_time = true;
    stream.record_row(data_row, inject_time);
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

fn parse_arrow_ipc_encapsulated_message(
    bytes: &[u8],
) -> Result<Box<dyn arrow2::array::Array>, String> {
    re_log::debug!(
        "parse_arrow_ipc_encapsulated_message: {} bytes",
        bytes.len()
    );

    use arrow2::io::ipc::read::{read_stream_metadata, StreamReader, StreamState};

    let mut cursor = std::io::Cursor::new(bytes);
    let metadata = match read_stream_metadata(&mut cursor) {
        Ok(metadata) => metadata,
        Err(err) => return Err(format!("Failed to read stream metadata: {err}")),
    };
    let stream = StreamReader::new(cursor, metadata, None);
    let chunks: Result<Vec<_>, _> = stream
        .map(|state| match state {
            Ok(StreamState::Some(chunk)) => Ok(chunk),
            Ok(StreamState::Waiting) => {
                unreachable!("cannot be waiting on a fixed buffer")
            }
            Err(err) => Err(err),
        })
        .collect();

    let chunks = chunks.map_err(|err| format!("Arrow error: {err}"))?;

    if chunks.is_empty() {
        return Err("No Chunk found in stream".to_owned());
    }
    if chunks.len() > 1 {
        return Err(format!(
            "Found {} chunks in stream - expected just one.",
            chunks.len()
        ));
    }
    let chunk = chunks.into_iter().next().unwrap();

    let arrays = chunk.into_arrays();

    assert_eq!(arrays.len(), 1); // TODO: error message

    Ok(arrays.into_iter().next().unwrap())
}

// ----------------------------------------------------------------------------
