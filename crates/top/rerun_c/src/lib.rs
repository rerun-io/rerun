//! The Rerun C SDK.
//!
//! The functions here must match `rerun_cpp/src/rerun/c/rerun.h`.

#![crate_type = "staticlib"]
#![allow(clippy::missing_safety_doc, clippy::undocumented_unsafe_blocks)] // Too much unsafe

mod arrow_utils;
mod component_type_registry;
mod error;
mod ptr;
mod recording_streams;
mod video;

use std::ffi::{c_char, c_uchar, CString};

use arrow::array::{ArrayRef as ArrowArrayRef, ListArray as ArrowListArray};
use arrow_utils::arrow_array_from_c_ffi;
use once_cell::sync::Lazy;

use re_arrow_util::ArrowArrayDowncastRef as _;
use re_sdk::{
    external::{nohash_hasher::IntMap, re_log_types::TimelineName},
    log::{Chunk, ChunkId, PendingRow, TimeColumn},
    time::TimeType,
    ComponentDescriptor, EntityPath, IndexCell, RecordingStream, RecordingStreamBuilder, StoreKind,
    TimePoint, Timeline,
};

use component_type_registry::COMPONENT_TYPES;
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
///
/// See `rr_spawn_options` in the C header.
#[derive(Debug, Clone)]
#[repr(C)]
pub struct CSpawnOptions {
    pub port: u16,
    pub memory_limit: CStringView,
    pub hide_welcome_screen: bool,
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

        spawn_opts.wait_for_bind = true;

        if !self.memory_limit.is_empty() {
            spawn_opts.memory_limit = self.memory_limit.as_str("memory_limit")?.to_owned();
        }

        spawn_opts.hide_welcome_screen = self.hide_welcome_screen;

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
            CStoreKind::Recording => Self::Recording,
            CStoreKind::Blueprint => Self::Blueprint,
        }
    }
}

/// See `rr_store_info` in the C header.
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

/// See `rr_component_descriptor` in the C header.
#[repr(C)]
pub struct CComponentDescriptor {
    pub archetype_name: CStringView,
    pub archetype_field_name: CStringView,
    pub component_name: CStringView,
}

/// See `rr_component_type` in the C header.
#[repr(C)]
pub struct CComponentType {
    pub descriptor: CComponentDescriptor,
    pub schema: arrow2::ffi::ArrowSchema,
}

/// See `rr_component_batch` in the C header.
#[repr(C)]
pub struct CComponentBatch {
    pub component_type: CComponentTypeHandle,
    pub array: arrow2::ffi::ArrowArray,
}

#[repr(C)]
pub struct CDataRow {
    pub entity_path: CStringView,
    pub num_data_cells: u32,
    pub batches: *mut CComponentBatch,
}

/// See `rr_component_column` in the C header.
#[repr(C)]
pub struct CComponentColumns {
    pub component_type: CComponentTypeHandle,

    /// A `ListArray` with the datatype `List(component_type)`.
    pub array: arrow2::ffi::ArrowArray,
}

/// See `rr_sorting_status` in the C header.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CSortingStatus {
    Unknown = 0,
    Sorted = 1,
    Unsorted = 2,
}

impl CSortingStatus {
    fn is_sorted(&self) -> Option<bool> {
        match self {
            Self::Sorted => Some(true),
            Self::Unsorted => Some(false),
            Self::Unknown => None,
        }
    }
}

/// See `rr_time_type` in the C header.
/// Equivalent to Rust [`re_sdk::time::TimeType`].
#[repr(u32)]
#[derive(Debug, Clone, Copy)]
pub enum CTimeType {
    /// Used e.g. for frames in a film.
    Sequence = 1,

    /// Nanoseconds.
    Duration = 2,

    /// Nanoseconds since Unix epoch (1970-01-01 00:00:00 UTC).
    Timestamp = 3,
}

/// See `rr_timeline` in the C header.
/// Equivalent to Rust [`re_sdk::Timeline`].
#[repr(C)]
#[derive(Debug, Clone)]
pub struct CTimeline {
    /// The name of the timeline.
    pub name: CStringView,

    /// The type of the timeline.
    pub typ: CTimeType,
}

impl TryFrom<CTimeline> for Timeline {
    type Error = CError;

    fn try_from(timeline: CTimeline) -> Result<Self, CError> {
        let name = timeline.name.as_str("timeline.name")?;
        let typ = match timeline.typ {
            CTimeType::Sequence => TimeType::Sequence,
            // TODO(#8635): differentiate between duration and timestamp
            CTimeType::Duration | CTimeType::Timestamp => TimeType::Time,
        };
        Ok(Self::new(name, typ))
    }
}

/// See `rr_time_column` in the C header.
/// Equivalent to Rust [`re_sdk::log::TimeColumn`].
#[repr(C)]
pub struct CTimeColumn {
    pub timeline: CTimeline,

    /// Times, a primitive array of i64.
    pub times: arrow2::ffi::ArrowArray,

    /// The sorting order of the times array.
    pub sorting_status: CSortingStatus,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CErrorCode {
    Ok = 0,

    _CategoryArgument = 0x0000_00010,
    UnexpectedNullArgument,
    InvalidStringArgument,
    InvalidEnumValue,
    InvalidRecordingStreamHandle,
    InvalidSocketAddress,
    InvalidComponentTypeHandle,
    InvalidServerUrl = 0x0000_0001a,

    _CategoryRecordingStream = 0x0000_00100,
    RecordingStreamRuntimeFailure,
    RecordingStreamCreationFailure,
    RecordingStreamSaveFailure,
    RecordingStreamStdoutFailure,
    RecordingStreamSpawnFailure,
    RecordingStreamChunkValidationFailure,
    RecordingStreamPropertyFailure,

    _CategoryArrow = 0x0000_1000,
    ArrowFfiSchemaImportError,
    ArrowFfiArrayImportError,

    _CategoryUtilities = 0x0001_0000,
    VideoLoadError,

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
    static VERSION: Lazy<CString> = Lazy::new(|| {
        CString::new(re_sdk::build_info().version.to_string()).expect("CString::new failed")
    }); // unwrap: there won't be any NUL bytes in the string

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
    let CComponentDescriptor {
        archetype_name,
        archetype_field_name,
        component_name,
    } = &component_type.descriptor;

    let archetype_name = if !archetype_name.is_null() {
        Some(archetype_name.as_str("component_type.descriptor.archetype_name")?)
    } else {
        None
    };
    let archetype_field_name = if !archetype_field_name.is_null() {
        Some(archetype_field_name.as_str("component_type.descriptor.archetype_field_name")?)
    } else {
        None
    };
    let component_name = component_name.as_str("component_type.descriptor.component_name")?;

    let component_descr = ComponentDescriptor {
        archetype_name: archetype_name.map(Into::into),
        archetype_field_name: archetype_field_name.map(Into::into),
        component_name: component_name.into(),
    };

    let schema =
        unsafe { arrow2::ffi::import_field_from_c(&component_type.schema) }.map_err(|err| {
            CError::new(
                CErrorCode::ArrowFfiSchemaImportError,
                &format!("Failed to import ffi schema: {err}"),
            )
        })?;

    Ok(COMPONENT_TYPES
        .write()
        .register(component_descr, schema.data_type))
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
        .store_source(re_sdk::external::re_log_types::StoreSource::CSdk)
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

/// See `THREAD_LIFE_TRACKER` for more information.
struct TrivialTypeWithDrop;

impl Drop for TrivialTypeWithDrop {
    fn drop(&mut self) {
        // Try to ensure that drop doesn't get optimized away.
        std::hint::black_box(self);
    }
}

thread_local! {
    /// It can happen that we end up inside of [`rr_recording_stream_free`] during a thread shutdown.
    /// This happens either when:
    /// * the application shuts down, causing the destructor of globally defined recordings to be invoked
    ///   -> Not an issue, we likely already destroyed the recording list.
    /// * the user stored their C++ recording in a thread local variable, and then shut down the thread.
    ///   -> More problematic, since we can't access `RECORDING_STREAMS` now, meaning we leak the recording.
    ///      (we can't access it because we use channels internally which in turn use thread-local storage)
    /// In either case we have a problem, since destroying a recording bottoms out to some thread-local storage
    /// access inside of channels, causing a crash!
    ///
    /// So how do we figure out that our thread is shutting down?
    /// As of writing `std::thread::current()` panics if there's nothing on `std::sys_common::thread_info::current_thread()`.
    /// Unfortunately, `std::sys_common` is a private implementation detail!
    /// So instead, we try accessing a thread local variable and see if that's still possible.
    /// If not, then we assume that the thread is shutting down.
    ///
    /// Just any thread local variable will not do though!
    /// We need something that is guaranteed to be dropped with the thread shutting down.
    /// A simple integer value won't do that, `Box` works but seems wasteful, so we use a trivial type with a drop implementation.
    #[allow(clippy::unnecessary_box_returns)]
    pub static THREAD_LIFE_TRACKER: TrivialTypeWithDrop = const { TrivialTypeWithDrop };
}

#[allow(unsafe_code)]
#[no_mangle]
pub extern "C" fn rr_recording_stream_free(id: CRecordingStream) {
    if THREAD_LIFE_TRACKER.try_with(|_v| {}).is_ok() {
        if let Some(stream) = RECORDING_STREAMS.lock().remove(id) {
            stream.disconnect();
        }
    } else {
        // Yes, at least as of writing we can still log things in this state!
        re_log::debug!(
            "rr_recording_stream_free called on a thread that is shutting down and can no longer access thread locals. We can't handle this and have to ignore this call."
        );
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
fn rr_recording_stream_connect_grpc_impl(
    stream: CRecordingStream,
    url: CStringView,
    flush_timeout_sec: f32,
) -> Result<(), CError> {
    let stream = recording_stream(stream)?;

    let url = url.as_str("url")?;
    let flush_timeout = if flush_timeout_sec >= 0.0 {
        Some(std::time::Duration::from_secs_f32(flush_timeout_sec))
    } else {
        None
    };

    if let Err(err) = stream.connect_grpc_opts(url, flush_timeout) {
        return Err(CError::new(CErrorCode::InvalidServerUrl, &err.to_string()));
    }

    Ok(())
}

#[allow(unsafe_code)]
#[no_mangle]
pub extern "C" fn rr_recording_stream_connect_grpc(
    id: CRecordingStream,
    url: CStringView,
    flush_timeout_sec: f32,
    error: *mut CError,
) {
    if let Err(err) = rr_recording_stream_connect_grpc_impl(id, url, flush_timeout_sec) {
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
fn rr_recording_stream_set_index_impl(
    stream: CRecordingStream,
    timeline_name: CStringView,
    time_type: CTimeType,
    value: i64,
) -> Result<(), CError> {
    let timeline = timeline_name.as_str("timeline_name")?;
    let stream = recording_stream(stream)?;
    let time_type = match time_type {
        CTimeType::Sequence => TimeType::Sequence,
        // TODO(#8635): do different things for Duration and Timestamp
        CTimeType::Duration | CTimeType::Timestamp => TimeType::Time,
    };
    stream.set_index(timeline, IndexCell::new(time_type, value));
    Ok(())
}

#[allow(unsafe_code)]
#[no_mangle]
pub extern "C" fn rr_recording_stream_set_index(
    stream: CRecordingStream,
    timeline_name: CStringView,
    time_type: CTimeType,
    value: i64,
    error: *mut CError,
) {
    if let Err(err) = rr_recording_stream_set_index_impl(stream, timeline_name, time_type, value) {
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
    if let Some(stream) = RECORDING_STREAMS.lock().get(stream) {
        stream.reset_time();
    }
}

#[allow(unsafe_code)]
#[allow(clippy::result_large_err)]
#[allow(clippy::needless_pass_by_value)] // Conceptually we're consuming the data_row, as we take ownership of data it points to.
fn rr_recording_stream_log_impl(
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
        num_data_cells,
        batches,
    } = data_row;

    let entity_path = entity_path.as_str("entity_path")?;
    let entity_path = EntityPath::parse_forgiving(entity_path);

    let num_data_cells = num_data_cells as usize;
    re_log::debug!("rerun_log {entity_path:?}, num_data_cells: {num_data_cells}");

    let batches = unsafe { std::slice::from_raw_parts_mut(batches, num_data_cells) };

    let mut components = IntMap::default();
    {
        let component_type_registry = COMPONENT_TYPES.read();

        for batch in batches {
            let CComponentBatch {
                component_type,
                array,
            } = &batch;
            let component_type = component_type_registry.get(*component_type)?;
            let datatype = component_type.datatype.clone();
            let values = unsafe { arrow_array_from_c_ffi(array, datatype) }?;
            components.insert(component_type.descriptor.clone(), values);
        }
    }

    let row = PendingRow {
        row_id,
        timepoint: TimePoint::default(), // we use the one in the recording stream for now
        components,
    };

    stream.record_row(entity_path, row, inject_time);

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
    if let Err(err) = rr_recording_stream_log_impl(stream, data_row, inject_time) {
        err.write_error(error);
    }
}

#[allow(unsafe_code)]
#[allow(clippy::result_large_err)]
fn rr_recording_stream_log_file_from_path_impl(
    stream: CRecordingStream,
    filepath: CStringView,
    entity_path_prefix: CStringView,
    static_: bool,
) -> Result<(), CError> {
    let stream = recording_stream(stream)?;

    let filepath = filepath.as_str("filepath")?;
    let entity_path_prefix = entity_path_prefix.as_str("entity_path_prefix").ok();

    stream
        .log_file_from_path(filepath, entity_path_prefix.map(Into::into), static_)
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
    static_: bool,
    error: *mut CError,
) {
    if let Err(err) =
        rr_recording_stream_log_file_from_path_impl(stream, filepath, entity_path_prefix, static_)
    {
        err.write_error(error);
    }
}

#[allow(unsafe_code)]
#[allow(clippy::result_large_err)]
fn rr_recording_stream_log_file_from_contents_impl(
    stream: CRecordingStream,
    filepath: CStringView,
    contents: CBytesView,
    entity_path_prefix: CStringView,
    static_: bool,
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
            static_,
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
    static_: bool,
    error: *mut CError,
) {
    if let Err(err) = rr_recording_stream_log_file_from_contents_impl(
        stream,
        filepath,
        contents,
        entity_path_prefix,
        static_,
    ) {
        err.write_error(error);
    }
}

#[allow(unsafe_code)]
#[allow(clippy::result_large_err)]
fn rr_recording_stream_send_columns_impl(
    stream: CRecordingStream,
    entity_path: CStringView,
    time_columns: &[CTimeColumn],
    component_columns: &[CComponentColumns],
) -> Result<(), CError> {
    // Create chunk-id as early as possible. It has a timestamp and is used to estimate e2e latency.
    let id = ChunkId::new();

    let stream = recording_stream(stream)?;
    let entity_path = entity_path.as_str("entity_path")?;

    let time_columns: IntMap<TimelineName, TimeColumn> = time_columns
        .iter()
        .map(|time_column| {
            let timeline: Timeline = time_column.timeline.clone().try_into()?;
            let datatype = arrow2::datatypes::DataType::Int64;
            let time_values_untyped = unsafe { arrow_array_from_c_ffi(&time_column.times, datatype) }?;
            let time_values = TimeColumn::read_array(&ArrowArrayRef::from(time_values_untyped)).map_err(|err| {
                CError::new(
                    CErrorCode::ArrowFfiArrayImportError,
                    &format!("Arrow C FFI import did not produce a Int64 time array - please file an issue at https://github.com/rerun-io/rerun/issues if you see this! This shouldn't be possible since conversion from C was successful with this datatype. Details: {err}")
                )
            })?;

            Ok((
                *timeline.name(),
                TimeColumn::new(
                    time_column.sorting_status.is_sorted(),
                    timeline,
                    time_values.clone(),
                ),
            ))
        })
        .collect::<Result<_, CError>>()?;

    let components: IntMap<ComponentDescriptor, ArrowListArray> = {
        let component_type_registry = COMPONENT_TYPES.read();
        component_columns
            .iter()
            .map(|batch| {
                let CComponentColumns {
                    component_type,
                    array,
                } = &batch;
                let component_type = component_type_registry.get(*component_type)?;
                let datatype = arrow2::array::ListArray::<i32>::default_datatype(
                    component_type.datatype.clone(),
                );

                let component_values_untyped = unsafe { arrow_array_from_c_ffi(array, datatype) }?;
                let component_values = component_values_untyped
                    .downcast_array_ref::<ArrowListArray>()
                    .ok_or_else(|| {
                        CError::new(
                            CErrorCode::ArrowFfiArrayImportError,
                            "Arrow C FFI import did not produce a ListArray - please file an issue at https://github.com/rerun-io/rerun/issues if you see this! This shouldn't be possible since conversion from C was successful with this datatype.",
                        )
                    })?;

                Ok((component_type.descriptor.clone(), component_values.clone()))
            })
            .collect::<Result<_, CError>>()?
    };

    let chunk = Chunk::from_auto_row_ids(
        id,
        entity_path.into(),
        time_columns,
        components.into_iter().collect(),
    )
    .map_err(|err| {
        CError::new(
            CErrorCode::RecordingStreamChunkValidationFailure,
            &format!("Failed to create chunk: {err}"),
        )
    })?;

    stream.send_chunk(chunk);

    Ok(())
}

#[allow(unsafe_code)]
#[no_mangle]
pub unsafe extern "C" fn rr_recording_stream_send_columns(
    stream: CRecordingStream,
    entity_path: CStringView,
    time_columns: *const CTimeColumn,
    num_time_columns: u32,
    component_batches: *const CComponentColumns,
    num_component_batches: u32,
    error: *mut CError,
) {
    let time_columns =
        unsafe { std::slice::from_raw_parts(time_columns, num_time_columns as usize) };
    let component_batches =
        unsafe { std::slice::from_raw_parts(component_batches, num_component_batches as usize) };

    if let Err(err) =
        rr_recording_stream_send_columns_impl(stream, entity_path, time_columns, component_batches)
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
