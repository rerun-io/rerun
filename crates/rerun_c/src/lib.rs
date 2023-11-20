//! The Rerun C SDK.
//!
//! The functions here must match `rerun.h`.

#![crate_type = "staticlib"]
#![allow(clippy::missing_safety_doc, clippy::undocumented_unsafe_blocks)] // Too much unsafe

mod error;
mod ptr;

use std::ffi::{c_char, CString};

use once_cell::sync::Lazy;
use parking_lot::Mutex;

use re_sdk::{
    external::re_log_types::{self},
    log::{DataCell, DataRow},
    ComponentName, EntityPath, RecordingStream, RecordingStreamBuilder, StoreKind, TimePoint,
};

// ----------------------------------------------------------------------------
// Types:

/// This is called `rr_string` in the C API.
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct CStringView {
    pub string: *const c_char,
    pub length: u32,
}

impl CStringView {
    #[allow(clippy::result_large_err)]
    pub fn as_str<'a>(&'a self, argument_name: &'a str) -> Result<&'a str, CError> {
        ptr::try_char_ptr_as_str(self.string, self.length, argument_name)
    }

    pub fn is_null(&self) -> bool {
        self.string.is_null()
    }
}

type CRecordingStream = u32;

/// C version of [`re_sdk::SpawnOptions`].
#[derive(Debug, Clone)]
#[repr(C)]
pub struct CSpawnOptions {
    pub port: u16,
    pub memory_limit: CStringView,
    pub executable_name: CStringView,
    pub executable_path: CStringView,
}

impl CSpawnOptions {
    #[allow(clippy::result_large_err)]
    pub fn as_rust(&self) -> Result<re_sdk::SpawnOptions, CError> {
        let mut spawn_opts = re_sdk::SpawnOptions::default();

        if self.port != 0 {
            spawn_opts.port = self.port;
        }

        if !self.memory_limit.is_null() {
            spawn_opts.memory_limit = self.memory_limit.as_str("memory_limit")?.to_owned();
        }

        if !self.executable_name.is_null() {
            spawn_opts.executable_name = self.executable_name.as_str("executable_name")?.to_owned();
        }

        if !self.executable_path.is_null() {
            spawn_opts.executable_path =
                Some(self.executable_path.as_str("executable_path")?.to_owned());
        }

        Ok(spawn_opts)
    }
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CStoreKind {
    /// A recording of user-data.
    Recording = 1,

    /// Data associated with the blueprint state.
    Blueprint = 2,
}

impl From<CStoreKind> for StoreKind {
    fn from(kind: CStoreKind) -> Self {
        match kind {
            CStoreKind::Recording => StoreKind::Recording,
            CStoreKind::Blueprint => StoreKind::Blueprint,
        }
    }
}

/// Simple C version of [`CStoreInfo`]
#[repr(C)]
#[derive(Debug)]
pub struct CStoreInfo {
    /// The user-chosen name of the application doing the logging.
    pub application_id: CStringView,

    pub store_kind: CStoreKind,
}

#[repr(C)]
pub struct CDataCell {
    pub component_name: CStringView,

    pub array: arrow2::ffi::ArrowArray,

    /// TODO(andreas): Use a schema registry.
    pub schema: arrow2::ffi::ArrowSchema,
}

#[repr(C)]
pub struct CDataRow {
    pub entity_path: CStringView,
    pub num_instances: u32,
    pub num_data_cells: u32,
    pub data_cells: *const CDataCell,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CErrorCode {
    Ok = 0,

    _CategoryArgument = 0x0000_00010,
    UnexpectedNullArgument,
    InvalidStringArgument,
    InvalidRecordingStreamHandle,
    InvalidSocketAddress,

    _CategoryRecordingStream = 0x0000_00100,
    RecordingStreamCreationFailure,
    RecordingStreamSaveFailure,
    // TODO(cmc): Really this should be its own category…
    RecordingStreamSpawnFailure,

    _CategoryArrow = 0x0000_1000,
    ArrowIpcMessageParsingFailure,
    ArrowDataCellError,
    ArrowFfiSchemaImportError, // TODO: sync to C and C++
    ArrowFfiArrayImportError,

    Unknown = 0xFFFF_FFFF,
}

#[repr(C)]
#[derive(Clone)]
pub struct CError {
    pub code: CErrorCode,
    pub message: [c_char; Self::MAX_MESSAGE_SIZE_BYTES],
}

// ----------------------------------------------------------------------------
// Global data:

const RERUN_REC_STREAM_CURRENT_RECORDING: CRecordingStream = 0xFFFFFFFF;
const RERUN_REC_STREAM_CURRENT_BLUEPRINT: CRecordingStream = 0xFFFFFFFE;

#[derive(Default)]
pub struct RecStreams {
    next_id: CRecordingStream,
    streams: ahash::HashMap<CRecordingStream, RecordingStream>,
}

impl RecStreams {
    fn insert(&mut self, stream: RecordingStream) -> CRecordingStream {
        let id = self.next_id;
        self.next_id += 1;
        self.streams.insert(id, stream);
        id
    }

    fn get(&self, id: CRecordingStream) -> Option<RecordingStream> {
        match id {
            RERUN_REC_STREAM_CURRENT_RECORDING => RecordingStream::get(StoreKind::Recording, None)
                .or(Some(RecordingStream::disabled())),
            RERUN_REC_STREAM_CURRENT_BLUEPRINT => RecordingStream::get(StoreKind::Blueprint, None)
                .or(Some(RecordingStream::disabled())),
            _ => self.streams.get(&id).cloned(),
        }
    }

    fn remove(&mut self, id: CRecordingStream) -> Option<RecordingStream> {
        match id {
            RERUN_REC_STREAM_CURRENT_BLUEPRINT | RERUN_REC_STREAM_CURRENT_RECORDING => None,
            _ => self.streams.remove(&id),
        }
    }
}

/// All recording streams created from C.
static RECORDING_STREAMS: Lazy<Mutex<RecStreams>> = Lazy::new(Mutex::default);

/// Access a C created recording stream.
#[allow(clippy::result_large_err)]
fn recording_stream(stream: CRecordingStream) -> Result<RecordingStream, CError> {
    RECORDING_STREAMS
        .lock()
        .get(stream)
        .ok_or(CError::invalid_recording_stream_handle())
}

// ----------------------------------------------------------------------------
// Public functions:

// SAFETY: the unsafety comes from #[no_mangle], because we can declare multiple
// functions with the same symbol names, and the linker behavior in this case i undefined.
#[allow(unsafe_code)]
#[no_mangle]
pub extern "C" fn rr_version_string() -> *const c_char {
    static VERSION: Lazy<CString> =
        Lazy::new(|| CString::new(re_sdk::build_info().to_string()).unwrap()); // unwrap: there won't be any NUL bytes in the string

    VERSION.as_ptr()
}

#[allow(clippy::result_large_err)]
fn rr_spawn_impl(spawn_opts: *const CSpawnOptions) -> Result<(), CError> {
    let spawn_opts = if spawn_opts.is_null() {
        re_sdk::SpawnOptions::default()
    } else {
        let spawn_opts = ptr::try_ptr_as_ref(spawn_opts, "spawn_opts")?;
        spawn_opts.as_rust()?
    };

    re_sdk::spawn(&spawn_opts)
        .map_err(|err| CError::new(CErrorCode::RecordingStreamSpawnFailure, &err.to_string()))?;

    Ok(())
}

#[allow(unsafe_code)]
#[no_mangle]
pub extern "C" fn rr_spawn(spawn_opts: *const CSpawnOptions, error: *mut CError) {
    if let Err(err) = rr_spawn_impl(spawn_opts) {
        err.write_error(error);
    }
}

#[allow(clippy::result_large_err)]
fn rr_recording_stream_new_impl(
    store_info: *const CStoreInfo,
    default_enabled: bool,
) -> Result<CRecordingStream, CError> {
    initialize_logging();

    let store_info = ptr::try_ptr_as_ref(store_info, "store_info")?;

    let CStoreInfo {
        application_id,
        store_kind,
    } = *store_info;

    let application_id = application_id.as_str("store_info.application_id")?;

    let mut rec_builder = RecordingStreamBuilder::new(application_id)
        //.is_official_example(is_official_example) // TODO(andreas): Is there a meaningful way to expose this?
        //.store_id(recording_id.clone()) // TODO(andreas): Expose store id.
        .store_source(re_log_types::StoreSource::CSdk)
        .default_enabled(default_enabled);

    if store_kind == CStoreKind::Blueprint {
        rec_builder = rec_builder.blueprint();
    }

    let rec = rec_builder.buffered().map_err(|err| {
        CError::new(
            CErrorCode::RecordingStreamCreationFailure,
            &format!("Failed to create recording stream: {err}"),
        )
    })?;
    Ok(RECORDING_STREAMS.lock().insert(rec))
}

#[allow(unsafe_code)]
#[no_mangle]
pub extern "C" fn rr_recording_stream_new(
    store_info: *const CStoreInfo,
    default_enabled: bool,
    error: *mut CError,
) -> CRecordingStream {
    match rr_recording_stream_new_impl(store_info, default_enabled) {
        Err(err) => {
            err.write_error(error);
            0
        }
        Ok(id) => id,
    }
}

#[allow(unsafe_code)]
#[no_mangle]
pub extern "C" fn rr_recording_stream_free(id: CRecordingStream) {
    if let Some(stream) = RECORDING_STREAMS.lock().remove(id) {
        stream.disconnect();
    }
}

#[allow(unsafe_code)]
#[no_mangle]
pub extern "C" fn rr_recording_stream_set_global(id: CRecordingStream, store_kind: CStoreKind) {
    let stream = RECORDING_STREAMS.lock().get(id);
    RecordingStream::set_global(store_kind.into(), stream);
}

#[allow(unsafe_code)]
#[no_mangle]
pub extern "C" fn rr_recording_stream_set_thread_local(
    id: CRecordingStream,
    store_kind: CStoreKind,
) {
    let stream = RECORDING_STREAMS.lock().get(id);
    RecordingStream::set_thread_local(store_kind.into(), stream);
}

#[allow(unsafe_code)]
#[no_mangle]
pub extern "C" fn rr_recording_stream_is_enabled(
    stream: CRecordingStream,
    error: *mut CError,
) -> bool {
    match rr_recording_stream_is_enabled_impl(stream) {
        Ok(enabled) => enabled,
        Err(err) => {
            err.write_error(error);
            false
        }
    }
}

#[allow(clippy::result_large_err)]
fn rr_recording_stream_is_enabled_impl(id: CRecordingStream) -> Result<bool, CError> {
    Ok(recording_stream(id)?.is_enabled())
}

#[allow(unsafe_code)]
#[no_mangle]
pub extern "C" fn rr_recording_stream_flush_blocking(id: CRecordingStream) {
    if let Some(stream) = RECORDING_STREAMS.lock().remove(id) {
        stream.flush_blocking();
    }
}

#[allow(clippy::result_large_err)]
fn rr_recording_stream_connect_impl(
    stream: CRecordingStream,
    tcp_addr: CStringView,
    flush_timeout_sec: f32,
) -> Result<(), CError> {
    let stream = recording_stream(stream)?;

    let tcp_addr = tcp_addr.as_str("tcp_addr")?;
    let tcp_addr = tcp_addr.parse().map_err(|err| {
        CError::new(
            CErrorCode::InvalidSocketAddress,
            &format!("Failed to parse tcp address {tcp_addr:?}: {err}"),
        )
    })?;

    let flush_timeout = if flush_timeout_sec >= 0.0 {
        Some(std::time::Duration::from_secs_f32(flush_timeout_sec))
    } else {
        None
    };
    stream.connect_opts(tcp_addr, flush_timeout);

    Ok(())
}

#[allow(unsafe_code)]
#[no_mangle]
pub extern "C" fn rr_recording_stream_connect(
    id: CRecordingStream,
    tcp_addr: CStringView,
    flush_timeout_sec: f32,
    error: *mut CError,
) {
    if let Err(err) = rr_recording_stream_connect_impl(id, tcp_addr, flush_timeout_sec) {
        err.write_error(error);
    }
}

#[allow(clippy::result_large_err)]
fn rr_recording_stream_spawn_impl(
    stream: CRecordingStream,
    spawn_opts: *const CSpawnOptions,
    flush_timeout_sec: f32,
) -> Result<(), CError> {
    let stream = recording_stream(stream)?;

    let spawn_opts = if spawn_opts.is_null() {
        re_sdk::SpawnOptions::default()
    } else {
        let spawn_opts = ptr::try_ptr_as_ref(spawn_opts, "spawn_opts")?;
        spawn_opts.as_rust()?
    };
    let flush_timeout = if flush_timeout_sec >= 0.0 {
        Some(std::time::Duration::from_secs_f32(flush_timeout_sec))
    } else {
        None
    };

    stream
        .spawn_opts(&spawn_opts, flush_timeout)
        .map_err(|err| CError::new(CErrorCode::RecordingStreamSpawnFailure, &err.to_string()))?;

    Ok(())
}

#[allow(unsafe_code)]
#[no_mangle]
pub extern "C" fn rr_recording_stream_spawn(
    id: CRecordingStream,
    spawn_opts: *const CSpawnOptions,
    flush_timeout_sec: f32,
    error: *mut CError,
) {
    if let Err(err) = rr_recording_stream_spawn_impl(id, spawn_opts, flush_timeout_sec) {
        err.write_error(error);
    }
}

#[allow(clippy::result_large_err)]
fn rr_recording_stream_save_impl(
    stream: CRecordingStream,
    path: CStringView,
) -> Result<(), CError> {
    let path = path.as_str("path")?;
    recording_stream(stream)?.save(path).map_err(|err| {
        CError::new(
            CErrorCode::RecordingStreamSaveFailure,
            &format!("Failed to save recording stream to {path:?}: {err}"),
        )
    })
}

#[allow(unsafe_code)]
#[no_mangle]
pub extern "C" fn rr_recording_stream_save(
    id: CRecordingStream,
    path: CStringView,
    error: *mut CError,
) {
    if let Err(err) = rr_recording_stream_save_impl(id, path) {
        err.write_error(error);
    }
}

#[allow(clippy::result_large_err)]
fn rr_recording_stream_set_time_sequence_impl(
    stream: CRecordingStream,
    timeline_name: CStringView,
    sequence: i64,
) -> Result<(), CError> {
    let timeline = timeline_name.as_str("timeline_name")?;
    recording_stream(stream)?.set_time_sequence(timeline, sequence);
    Ok(())
}

#[allow(unsafe_code)]
#[no_mangle]
pub extern "C" fn rr_recording_stream_set_time_sequence(
    stream: CRecordingStream,
    timeline_name: CStringView,
    sequence: i64,
    error: *mut CError,
) {
    if let Err(err) = rr_recording_stream_set_time_sequence_impl(stream, timeline_name, sequence) {
        err.write_error(error);
    }
}

#[allow(clippy::result_large_err)]
fn rr_recording_stream_set_time_seconds_impl(
    stream: CRecordingStream,
    timeline_name: CStringView,
    seconds: f64,
) -> Result<(), CError> {
    let timeline = timeline_name.as_str("timeline_name")?;
    recording_stream(stream)?.set_time_seconds(timeline, seconds);
    Ok(())
}

#[allow(unsafe_code)]
#[no_mangle]
pub extern "C" fn rr_recording_stream_set_time_seconds(
    stream: CRecordingStream,
    timeline_name: CStringView,
    seconds: f64,
    error: *mut CError,
) {
    if let Err(err) = rr_recording_stream_set_time_seconds_impl(stream, timeline_name, seconds) {
        err.write_error(error);
    }
}

#[allow(clippy::result_large_err)]
fn rr_recording_stream_set_time_nanos_impl(
    stream: CRecordingStream,
    timeline_name: CStringView,
    nanos: i64,
) -> Result<(), CError> {
    let timeline = timeline_name.as_str("timeline_name")?;
    recording_stream(stream)?.set_time_nanos(timeline, nanos);
    Ok(())
}

#[allow(unsafe_code)]
#[no_mangle]
pub extern "C" fn rr_recording_stream_set_time_nanos(
    stream: CRecordingStream,
    timeline_name: CStringView,
    nanos: i64,
    error: *mut CError,
) {
    if let Err(err) = rr_recording_stream_set_time_nanos_impl(stream, timeline_name, nanos) {
        err.write_error(error);
    }
}

#[allow(unsafe_code)]
#[allow(clippy::result_large_err)]
fn rr_recording_stream_disable_timeline_impl(
    stream: CRecordingStream,
    timeline_name: CStringView,
) -> Result<(), CError> {
    let timeline = timeline_name.as_str("timeline_name")?;
    recording_stream(stream)?.disable_timeline(timeline);
    Ok(())
}

#[allow(unsafe_code)]
#[no_mangle]
pub extern "C" fn rr_recording_stream_disable_timeline(
    stream: CRecordingStream,
    timeline_name: CStringView,
    error: *mut CError,
) {
    if let Err(err) = rr_recording_stream_disable_timeline_impl(stream, timeline_name) {
        err.write_error(error);
    }
}

#[allow(unsafe_code)]
#[no_mangle]
pub extern "C" fn rr_recording_stream_reset_time(stream: CRecordingStream) {
    if let Some(stream) = RECORDING_STREAMS.lock().remove(stream) {
        stream.reset_time();
    }
}

#[allow(unsafe_code)]
#[allow(clippy::result_large_err)]
fn rr_log_impl(
    stream: CRecordingStream,
    data_row: *const CDataRow,
    inject_time: bool,
) -> Result<(), CError> {
    let stream = recording_stream(stream)?;

    let data_row = ptr::try_ptr_as_ref(data_row, "data_row")?;

    let CDataRow {
        entity_path,
        num_instances,
        num_data_cells,
        data_cells,
    } = *data_row;

    let entity_path = entity_path.as_str("entity_path")?;
    let entity_path = EntityPath::parse_forgiving(entity_path);

    re_log::debug!(
        "rerun_log {entity_path:?}, num_instances: {num_instances}, num_data_cells: {num_data_cells}",
    );

    let mut cells = re_log_types::DataCellVec::default();
    cells.reserve(num_data_cells as usize);

    let data_cells = unsafe { std::slice::from_raw_parts(data_cells, num_data_cells as usize) };

    for data_cell in data_cells {
        let CDataCell {
            component_name,
            array,
            schema,
        } = &data_cell;

        let component_name = component_name.as_str("data_cells[i].component_name")?;
        let component_name = ComponentName::from(component_name);

        // let bytes = unsafe { std::slice::from_raw_parts(bytes, num_bytes as usize) };
        // let array = parse_arrow_ipc_encapsulated_message(bytes).map_err(|err| {
        //     CError::new(
        //         CErrorCode::ArrowIpcMessageParsingFailure,
        //         &format!("Failed to parse Arrow IPC encapsulated message: {err}"),
        //     )
        // })?;

        let field = unsafe { arrow2::ffi::import_field_from_c(schema) }.map_err(|err| {
            CError::new(
                CErrorCode::ArrowFfiSchemaImportError,
                &format!("Failed to import ffi schema: {err}"),
            )
        })?;

        // We need to copy the array since `import_array_from_c` takes ownership of the array.
        // Since we don't touch the original afterwards anymore and array doesn't have a drop implementation,
        // it should be safe to do so.
        let ffi_array: arrow2::ffi::ArrowArray = unsafe { std::mem::transmute_copy(array) };

        let values = unsafe { arrow2::ffi::import_array_from_c(ffi_array, field.data_type) }
            .map_err(|err| {
                CError::new(
                    CErrorCode::ArrowFfiArrayImportError,
                    &format!("Failed to import ffi array: {err}"),
                )
            })?;

        cells.push(
            DataCell::try_from_arrow(component_name, values).map_err(|err| {
                CError::new(
                    CErrorCode::ArrowDataCellError,
                    &format!("Failed to create arrow datacell: {err}"),
                )
            })?,
        );
    }

    let data_row = DataRow::from_cells(
        re_sdk::log::RowId::random(),
        TimePoint::default(), // we use the one in the recording stream for now
        entity_path,
        num_instances,
        cells,
    )
    .map_err(|err| {
        CError::new(
            CErrorCode::ArrowDataCellError,
            &format!("Failed to create DataRow from CDataRow: {err}"),
        )
    })?;

    stream.record_row(data_row, inject_time);

    Ok(())
}

#[allow(unsafe_code)]
#[no_mangle]
pub unsafe extern "C" fn rr_recording_stream_log(
    stream: CRecordingStream,
    data_row: *const CDataRow,
    inject_time: bool,
    error: *mut CError,
) {
    if let Err(err) = rr_log_impl(stream, data_row, inject_time) {
        err.write_error(error);
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

    // This IPC message represents the contents of a single DataCell, thus we should have a single
    // field.
    if metadata.schema.fields.len() != 1 {
        return Err(format!(
            "Found {} fields in stream metadata - expected exactly one.",
            metadata.schema.fields.len(),
        ));
    }
    // Might need that later if it turns out we don't have any data to log.
    let datatype = metadata.schema.fields[0].data_type().clone();

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

    // We're not sending a `DataCellColumn`'s (i.e. `List<DataCell>`) worth of data like we normally do
    // here, rather we're sending a single, independent `DataCell`'s worth of data.
    //
    // This distinction is crucial:
    // - The data for a `DataCellColumn` containing a single empty `DataCell` is a unit-length list-array whose
    //   first and only entry is an empty array (`ListArray[[]]`). There's actually data there (as
    //   in bytes).
    // - The data for a standalone empty `DataCell`, on the other hand, is literally nothing. It's
    //   zero bytes.
    //
    // Where there's no data whatsoever, the chunk gets optimized out, which is why logging an
    // empty array in C++ ends up hitting this path.
    if chunks.is_empty() {
        // The fix is simple: craft an empty array with the correct datatype.
        return Ok(arrow2::array::new_empty_array(datatype));
    }

    if chunks.len() > 1 {
        return Err(format!(
            "Found {} chunks in stream - expected just one.",
            chunks.len()
        ));
    }
    let chunk = chunks.into_iter().next().unwrap();

    let arrays = chunk.into_arrays();

    if arrays.len() != 1 {
        return Err(format!("Expected one array, got {}", arrays.len()));
    }

    Ok(arrays.into_iter().next().unwrap())
}
