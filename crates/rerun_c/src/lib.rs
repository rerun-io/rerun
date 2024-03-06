//! The Rerun C SDK.
//!
//! The functions here must match `rerun.h`.

#![crate_type = "staticlib"]
#![allow(clippy::missing_safety_doc, clippy::undocumented_unsafe_blocks)] // Too much unsafe

mod component_type_registry;
mod error;
mod ptr;
mod recording_streams;

use std::ffi::{c_char, c_uchar, CString};

use component_type_registry::COMPONENT_TYPES;
use once_cell::sync::Lazy;

use re_sdk::{
    external::re_log_types::{self},
    log::{DataCell, DataRow},
    ComponentName, EntityPath, RecordingStream, RecordingStreamBuilder, StoreKind, TimePoint,
};
use recording_streams::{recording_stream, RECORDING_STREAMS};

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

    pub fn is_empty(&self) -> bool {
        self.length == 0
    }
}

/// This is called `rr_bytes` in the C API.
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct CBytesView {
    pub bytes: *const c_uchar,
    pub length: u32,
}

impl CBytesView {
    #[allow(clippy::result_large_err)]
    pub fn as_bytes<'a>(&self, argument_name: &'a str) -> Result<&'a [u8], CError> {
        ptr::try_ptr_as_slice(self.bytes, self.length, argument_name)
    }

    pub fn is_null(&self) -> bool {
        self.bytes.is_null()
    }

    pub fn is_empty(&self) -> bool {
        self.length == 0
    }
}

pub type CRecordingStream = u32;

pub type CComponentTypeHandle = u32;

pub const RR_REC_STREAM_CURRENT_RECORDING: CRecordingStream = 0xFFFFFFFF;
pub const RR_REC_STREAM_CURRENT_BLUEPRINT: CRecordingStream = 0xFFFFFFFE;
pub const RR_COMPONENT_TYPE_HANDLE_INVALID: CComponentTypeHandle = 0xFFFFFFFF;

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

        if !self.memory_limit.is_empty() {
            spawn_opts.memory_limit = self.memory_limit.as_str("memory_limit")?.to_owned();
        }

        if !self.executable_name.is_empty() {
            spawn_opts.executable_name = self.executable_name.as_str("executable_name")?.to_owned();
        }

        if !self.executable_path.is_empty() {
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

    /// The user-chosen name of the recording being logged to.
    ///
    /// Defaults to a random ID if unspecified.
    pub recording_id: CStringView,

    pub store_kind: CStoreKind,
}

#[repr(C)]
pub struct CComponentType {
    pub name: CStringView,
    pub schema: arrow2::ffi::ArrowSchema,
}

#[repr(C)]
pub struct CDataCell {
    pub component_type: CComponentTypeHandle,
    pub array: arrow2::ffi::ArrowArray,
}

#[repr(C)]
pub struct CDataRow {
    pub entity_path: CStringView,
    pub num_instances: u32,
    pub num_data_cells: u32,
    pub data_cells: *mut CDataCell,
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
    InvalidComponentTypeHandle,

    _CategoryRecordingStream = 0x0000_00100,
    RecordingStreamRuntimeFailure,
    RecordingStreamCreationFailure,
    RecordingStreamSaveFailure,
    RecordingStreamStdoutFailure,
    RecordingStreamSpawnFailure,

    _CategoryArrow = 0x0000_1000,
    ArrowFfiSchemaImportError,
    ArrowFfiArrayImportError,
    ArrowDataCellError,

    Unknown = 0xFFFF_FFFF,
}

#[repr(C)]
#[derive(Clone)]
pub struct CError {
    pub code: CErrorCode,
    pub message: [c_char; Self::MAX_MESSAGE_SIZE_BYTES],
}

// ----------------------------------------------------------------------------
// Public functions:

// SAFETY: the unsafety comes from #[no_mangle], because we can declare multiple
// functions with the same symbol names, and the linker behavior in this case i undefined.
#[allow(unsafe_code)]
#[no_mangle]
pub extern "C" fn rr_version_string() -> *const c_char {
    static VERSION: Lazy<CString> =
        Lazy::new(|| CString::new(re_sdk::build_info().version.to_string()).unwrap()); // unwrap: there won't be any NUL bytes in the string

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
#[allow(unsafe_code)]
fn rr_register_component_type_impl(
    component_type: &CComponentType,
) -> Result<CComponentTypeHandle, CError> {
    let component_name = component_type.name.as_str("component_type.name")?;
    let component_name = ComponentName::from(component_name);
    let schema =
        unsafe { arrow2::ffi::import_field_from_c(&component_type.schema) }.map_err(|err| {
            CError::new(
                CErrorCode::ArrowFfiSchemaImportError,
                &format!("Failed to import ffi schema: {err}"),
            )
        })?;

    Ok(COMPONENT_TYPES
        .write()
        .register(component_name, schema.data_type))
}

#[allow(unsafe_code)]
#[no_mangle]
pub extern "C" fn rr_register_component_type(
    // Note that since this is passed by value, Arrow2 will release the schema on drop!
    component_type: CComponentType,
    error: *mut CError,
) -> u32 {
    match rr_register_component_type_impl(&component_type) {
        Ok(id) => id,
        Err(err) => {
            err.write_error(error);
            RR_COMPONENT_TYPE_HANDLE_INVALID
        }
    }
}

#[allow(clippy::result_large_err)]
fn rr_recording_stream_new_impl(
    store_info: *const CStoreInfo,
    default_enabled: bool,
) -> Result<CRecordingStream, CError> {
    re_log::setup_logging();

    let store_info = ptr::try_ptr_as_ref(store_info, "store_info")?;

    let CStoreInfo {
        application_id,
        recording_id,
        store_kind,
    } = *store_info;

    let application_id = application_id.as_str("store_info.application_id")?;

    let mut rec_builder = RecordingStreamBuilder::new(application_id)
        //.store_id(recording_id.clone()) // TODO(andreas): Expose store id.
        .store_source(re_log_types::StoreSource::CSdk)
        .default_enabled(default_enabled);

    if !(recording_id.is_null() || recording_id.is_empty()) {
        if let Ok(recording_id) = recording_id.as_str("recording_id") {
            rec_builder = rec_builder.recording_id(recording_id);
        }
    }

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
fn rr_recording_stream_stdout_impl(stream: CRecordingStream) -> Result<(), CError> {
    recording_stream(stream)?.stdout().map_err(|err| {
        CError::new(
            CErrorCode::RecordingStreamStdoutFailure,
            &format!("Failed to forward recording stream to stdout: {err}"),
        )
    })
}

#[allow(unsafe_code)]
#[no_mangle]
pub extern "C" fn rr_recording_stream_stdout(id: CRecordingStream, error: *mut CError) {
    if let Err(err) = rr_recording_stream_stdout_impl(id) {
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
#[allow(clippy::needless_pass_by_value)] // Conceptually we're consuming the data_row, as we take ownership of data it points to.
fn rr_log_impl(
    stream: CRecordingStream,
    data_row: CDataRow,
    inject_time: bool,
) -> Result<(), CError> {
    // Create row-id as early as possible. It has a timestamp and is used to estimate e2e latency.
    // TODO(emilk): move to before we arrow-serialize the data
    let row_id = re_sdk::log::RowId::new();

    let stream = recording_stream(stream)?;

    let CDataRow {
        entity_path,
        num_instances,
        num_data_cells,
        data_cells,
    } = data_row;

    let entity_path = entity_path.as_str("entity_path")?;
    let entity_path = EntityPath::parse_forgiving(entity_path);

    let num_data_cells = num_data_cells as usize;
    re_log::debug!(
        "rerun_log {entity_path:?}, num_instances: {num_instances}, num_data_cells: {num_data_cells}",
    );

    let mut cells = re_log_types::DataCellVec::default();
    cells.reserve(num_data_cells);

    let data_cells = unsafe { std::slice::from_raw_parts_mut(data_cells, num_data_cells) };

    {
        let component_type_registry = COMPONENT_TYPES.read();

        for data_cell in data_cells {
            // Arrow2 implements drop for ArrowArray and ArrowSchema.
            //
            // Therefore, for things to work correctly we have to take ownership of the data cell!
            // The C interface is documented to take ownership of the data cell - the user should NOT call `release`.
            // This makes sense because from here on out we want to manage the lifetime of the underlying schema and array data:
            // the schema won't survive a loop iteration since it's reference passed for import, whereas the ArrowArray lives
            // on a longer within the resulting arrow::Array.
            let CDataCell {
                component_type,
                array,
            } = unsafe { std::ptr::read(data_cell) };

            // It would be nice to now mark the data_cell as "consumed" by setting the original release method to nullptr.
            // This would signifies to the calling code that the data_cell is no longer owned.
            // However, Arrow2 doesn't allow us to access the fields of the ArrowArray and ArrowSchema structs.

            let component_type = component_type_registry.get(component_type).ok_or_else(|| {
                CError::new(
                    CErrorCode::InvalidComponentTypeHandle,
                    &format!("Invalid component type handle: {component_type}"),
                )
            })?;

            let values =
                unsafe { arrow2::ffi::import_array_from_c(array, component_type.datatype.clone()) }
                    .map_err(|err| {
                        CError::new(
                            CErrorCode::ArrowFfiArrayImportError,
                            &format!("Failed to import ffi array: {err}"),
                        )
                    })?;

            cells.push(
                DataCell::try_from_arrow(component_type.name, values).map_err(|err| {
                    CError::new(
                        CErrorCode::ArrowDataCellError,
                        &format!("Failed to create arrow datacell: {err}"),
                    )
                })?,
            );
        }
    }

    let data_row = DataRow::from_cells(
        row_id,
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
    data_row: CDataRow,
    inject_time: bool,
    error: *mut CError,
) {
    if let Err(err) = rr_log_impl(stream, data_row, inject_time) {
        err.write_error(error);
    }
}

#[allow(unsafe_code)]
#[allow(clippy::result_large_err)]
fn rr_log_file_from_path_impl(
    stream: CRecordingStream,
    filepath: CStringView,
    entity_path_prefix: CStringView,
    timeless: bool,
) -> Result<(), CError> {
    let stream = recording_stream(stream)?;

    let filepath = filepath.as_str("filepath")?;
    let entity_path_prefix = entity_path_prefix.as_str("entity_path_prefix").ok();

    stream
        .log_file_from_path(filepath, entity_path_prefix.map(Into::into), timeless)
        .map_err(|err| {
            CError::new(
                CErrorCode::RecordingStreamRuntimeFailure,
                &format!("Couldn't load file {filepath:?}: {err}"),
            )
        })?;

    Ok(())
}

#[allow(unsafe_code)]
#[no_mangle]
pub unsafe extern "C" fn rr_recording_stream_log_file_from_path(
    stream: CRecordingStream,
    filepath: CStringView,
    entity_path_prefix: CStringView,
    timeless: bool,
    error: *mut CError,
) {
    if let Err(err) = rr_log_file_from_path_impl(stream, filepath, entity_path_prefix, timeless) {
        err.write_error(error);
    }
}

#[allow(unsafe_code)]
#[allow(clippy::result_large_err)]
fn rr_log_file_from_contents_impl(
    stream: CRecordingStream,
    filepath: CStringView,
    contents: CBytesView,
    entity_path_prefix: CStringView,
    timeless: bool,
) -> Result<(), CError> {
    let stream = recording_stream(stream)?;

    let filepath = filepath.as_str("filepath")?;
    let contents = contents.as_bytes("contents")?;
    let entity_path_prefix = entity_path_prefix.as_str("entity_path_prefix").ok();

    stream
        .log_file_from_contents(
            filepath,
            std::borrow::Cow::Borrowed(contents),
            entity_path_prefix.map(Into::into),
            timeless,
        )
        .map_err(|err| {
            CError::new(
                CErrorCode::RecordingStreamRuntimeFailure,
                &format!("Couldn't load file {filepath:?}: {err}"),
            )
        })?;

    Ok(())
}

#[allow(unsafe_code)]
#[no_mangle]
pub unsafe extern "C" fn rr_recording_stream_log_file_from_contents(
    stream: CRecordingStream,
    filepath: CStringView,
    contents: CBytesView,
    entity_path_prefix: CStringView,
    timeless: bool,
    error: *mut CError,
) {
    if let Err(err) =
        rr_log_file_from_contents_impl(stream, filepath, contents, entity_path_prefix, timeless)
    {
        err.write_error(error);
    }
}

// ----------------------------------------------------------------------------
// Private functions

#[allow(unsafe_code)]
#[no_mangle]
pub unsafe extern "C" fn _rr_escape_entity_path_part(part: CStringView) -> *const c_char {
    let Ok(part) = part.as_str("entity_path_part") else {
        return std::ptr::null();
    };

    let part = re_sdk::EntityPathPart::from(part).escaped_string();

    let Ok(part) = CString::new(part) else {
        return std::ptr::null();
    };

    part.into_raw()
}

#[allow(unsafe_code)]
#[no_mangle]
pub unsafe extern "C" fn _rr_free_string(str: *mut c_char) {
    if str.is_null() {
        return;
    }

    // Free the string:
    unsafe {
        // SAFETY: `_rr_free_string` should only be called on strings allocated by `_rr_escape_entity_path_part`.
        let _ = CString::from_raw(str);
    }
}
