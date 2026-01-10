//! The Rerun C SDK.
//!
//! The functions here must match `rerun_cpp/src/rerun/c/rerun.h`.

#![crate_type = "staticlib"]
#![expect(clippy::missing_safety_doc, clippy::undocumented_unsafe_blocks)] // Too much unsafe

mod arrow_utils;
mod component_type_registry;
mod error;
mod ptr;
mod recording_streams;
mod video;

use std::ffi::{CString, c_char, c_float, c_uchar};
use std::time::Duration;

use arrow::array::{ArrayRef as ArrowArrayRef, ListArray as ArrowListArray};
use arrow::ffi::{FFI_ArrowArray, FFI_ArrowSchema};
use arrow_utils::arrow_array_from_c_ffi;
use component_type_registry::COMPONENT_TYPES;
use re_arrow_util::ArrowArrayDowncastRef as _;
use re_sdk::external::nohash_hasher::IntMap;
use re_sdk::external::re_log_types::TimelineName;
use re_sdk::log::{Chunk, ChunkId, PendingRow, TimeColumn};
use re_sdk::time::TimeType;
use re_sdk::{
    ComponentDescriptor, EntityPath, RecordingStream, RecordingStreamBuilder, StoreKind, TimeCell,
    TimePoint, Timeline,
};
use recording_streams::{RECORDING_STREAMS, recording_stream};

// ----------------------------------------------------------------------------
// Types:

/// This is called `rr_string` in the C API.
///
/// NOTE: [`CStringView`] is NOT an `Option`, and there is no difference between null and "".
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct CStringView {
    pub string: *const c_char,
    pub length: u32,
}

impl CStringView {
    /// Error if the string is not valid UTF8, or is null and non-zero in length.
    ///
    /// May return the empty string.
    #[expect(clippy::result_large_err)]
    pub fn as_maybe_empty_str<'a>(&'a self, argument_name: &'a str) -> Result<&'a str, CError> {
        if self.is_empty() {
            Ok("")
        } else {
            debug_assert!(
                1000 < self.string.addr() && self.length < 1_000_000,
                "DEBUG ASSERT: Suspected memory corruption when reading argument {argument_name:?}: {self:#?}"
            );
            ptr::try_char_ptr_as_str(self.string, self.length, argument_name)
        }
    }

    /// Treat the empty string "" as None.
    #[expect(clippy::result_large_err)]
    pub fn as_optional_str<'a>(
        &'a self,
        argument_name: &'a str,
    ) -> Result<Option<&'a str>, CError> {
        if self.is_empty() {
            Ok(None)
        } else {
            self.as_nonempty_str(argument_name).map(Some)
        }
    }

    /// Error if the string was empty.
    #[expect(clippy::result_large_err)]
    pub fn as_nonempty_str<'a>(&'a self, argument_name: &'a str) -> Result<&'a str, CError> {
        if self.is_empty() {
            Err(CError::new(
                CErrorCode::InvalidStringArgument,
                &format!("{argument_name:?} was an empty string"),
            ))
        } else {
            self.as_maybe_empty_str(argument_name)
        }
    }

    /// Is this the "" string?
    ///
    /// NOTE: [`CStringView`] is NOT an `Option`, and there is no difference between null and "".
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
    #[expect(clippy::result_large_err)]
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
    pub server_memory_limit: CStringView,
    pub hide_welcome_screen: bool,
    pub detach_process: bool,
    pub executable_name: CStringView,
    pub executable_path: CStringView,
}

impl CSpawnOptions {
    #[expect(clippy::result_large_err)]
    pub fn as_rust(&self) -> Result<re_sdk::SpawnOptions, CError> {
        let Self {
            port,
            memory_limit,
            server_memory_limit,
            hide_welcome_screen,
            detach_process,
            executable_name,
            executable_path,
        } = self;

        let mut spawn_opts = re_sdk::SpawnOptions::default();

        if *port != 0 {
            spawn_opts.port = *port;
        }

        spawn_opts.wait_for_bind = true;

        if let Some(memory_limit) = memory_limit.as_optional_str("memory_limit")? {
            spawn_opts.memory_limit = memory_limit.to_owned();
        }
        if let Some(server_memory_limit) =
            server_memory_limit.as_optional_str("server_memory_limit")?
        {
            spawn_opts.server_memory_limit = server_memory_limit.to_owned();
        }

        spawn_opts.hide_welcome_screen = *hide_welcome_screen;
        spawn_opts.detach_process = *detach_process;

        if let Some(executable_name) = executable_name.as_optional_str("executable_name")? {
            spawn_opts.executable_name = executable_name.to_owned();
        }

        if let Some(executable_path) = executable_path.as_optional_str("executable_path")? {
            spawn_opts.executable_path = Some(executable_path.to_owned());
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
    pub component: CStringView,
    pub component_type: CStringView,
}

/// See `rr_component_type` in the C header.
#[repr(C)]
pub struct CComponentType {
    pub descriptor: CComponentDescriptor,
    pub schema: FFI_ArrowSchema,
}

/// See `rr_component_batch` in the C header.
#[repr(C)]
pub struct CComponentBatch {
    pub component_type: CComponentTypeHandle,
    pub array: FFI_ArrowArray,
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
    pub array: FFI_ArrowArray,
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
        let name = timeline.name.as_nonempty_str("timeline.name")?;
        let typ = match timeline.typ {
            CTimeType::Sequence => TimeType::Sequence,
            CTimeType::Duration => TimeType::DurationNs,
            CTimeType::Timestamp => TimeType::TimestampNs,
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
    pub times: FFI_ArrowArray,

    /// The sorting order of the times array.
    pub sorting_status: CSortingStatus,
}

/// Log sink which streams messages to a gRPC server.
///
/// The behavior of this sink is the same as the one set by `rr_recording_stream_connect_grpc`.
///
/// See `rr_grpc_sink` in the C header.
#[derive(Debug)]
#[repr(C)]
pub struct CGrpcSink {
    /// A Rerun gRPC URL
    ///
    /// Default is `rerun+http://127.0.0.1:9876/proxy`.
    pub url: CStringView,
}

/// Log sink which writes messages to a file.
///
/// See `rr_file_sink` in the C header.
#[derive(Debug)]
#[repr(C)]
pub struct CFileSink {
    /// Path to the output file.
    pub path: CStringView,
}

/// A sink for log messages.
///
/// See specific log sink types for more information:
/// * [`CGrpcSink`]
/// * [`CFileSink`]
///
/// See `rr_log_sink` and `RR_LOG_SINK_KIND` enum values in the C header.
///
/// Layout is defined in [the Rust reference](https://doc.rust-lang.org/stable/reference/type-layout.html#reprc-enums-with-fields).
#[derive(Debug)]
#[repr(C, u8)]
pub enum CLogSink {
    GrpcSink { grpc: CGrpcSink } = 0,
    FileSink { file: CFileSink } = 1,
}

// ⚠️ Remember to also update `uint32_t rr_error_code` AND `enum class ErrorCode` !
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CErrorCode {
    Ok = 0,
    OutOfMemory,
    NotImplemented,
    SdkVersionMismatch,

    // Invalid argument errors.
    _CategoryArgument = 0x0000_00010,
    UnexpectedNullArgument,
    InvalidStringArgument,
    InvalidEnumValue,
    InvalidRecordingStreamHandle,
    InvalidSocketAddress,
    InvalidComponentTypeHandle,
    InvalidTimeArgument,
    InvalidTensorDimension,
    InvalidComponent,
    InvalidServerUrl = 0x0000_0001a,
    FileRead,
    InvalidMemoryLimit,

    // Recording stream errors
    _CategoryRecordingStream = 0x0000_00100,
    RecordingStreamRuntimeFailure,
    RecordingStreamCreationFailure,
    RecordingStreamSaveFailure,
    RecordingStreamStdoutFailure,
    RecordingStreamSpawnFailure,
    RecordingStreamChunkValidationFailure,
    RecordingStreamServeGrpcFailure,
    RecordingStreamFlushTimeout,
    RecordingStreamFlushFailure,

    // Arrow data processing errors.
    _CategoryArrow = 0x0000_1000,
    ArrowFfiSchemaImportError,
    ArrowFfiArrayImportError,

    // Utility errors.
    _CategoryUtilities = 0x0001_0000,
    VideoLoadError,

    // Errors relating to file IO.
    _CategoryFileIO = 0x0010_0000,
    FileOpenFailure,

    // Errors directly translated from arrow::StatusCode.
    _CategoryArrowCppStatus = 0x1000_0000,

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
#[expect(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn rr_version_string() -> *const c_char {
    static VERSION: std::sync::LazyLock<CString> = std::sync::LazyLock::new(|| {
        CString::new(re_sdk::build_info().version.to_string()).expect("CString::new failed")
    }); // unwrap: there won't be any NUL bytes in the string

    VERSION.as_ptr()
}

#[expect(clippy::result_large_err)]
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

#[expect(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn rr_spawn(spawn_opts: *const CSpawnOptions, error: *mut CError) {
    if let Err(err) = rr_spawn_impl(spawn_opts) {
        err.write_error(error);
    }
}

#[expect(clippy::result_large_err)]
fn rr_register_component_type_impl(
    component_type: &CComponentType,
) -> Result<CComponentTypeHandle, CError> {
    let CComponentDescriptor {
        archetype_name,
        component,
        component_type: component_type_descr,
    } = &component_type.descriptor;

    let archetype_name =
        archetype_name.as_optional_str("component_type.descriptor.archetype_name")?;

    let component = component.as_nonempty_str("component_type.descriptor.component")?;

    let component_type_descr =
        component_type_descr.as_optional_str("component_type.descriptor.component_type")?;

    let component_descr = ComponentDescriptor {
        archetype: archetype_name.map(Into::into),
        component: component.into(),
        component_type: component_type_descr.map(Into::into),
    };

    let field = arrow::datatypes::Field::try_from(&component_type.schema).map_err(|err| {
        CError::new(
            CErrorCode::ArrowFfiSchemaImportError,
            &format!("Failed to import ffi schema: {err}"),
        )
    })?;

    Ok(COMPONENT_TYPES
        .write()
        .register(component_descr, field.data_type().clone()))
}

#[expect(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn rr_register_component_type(
    // Note that since this is passed by value, arrow will release the schema on drop!
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

#[expect(clippy::result_large_err)]
fn rr_recording_stream_new_impl(
    store_info: *const CStoreInfo,
    default_enabled: bool,
) -> Result<CRecordingStream, CError> {
    {
        use std::sync::Once;
        static INIT: Once = Once::new();
        INIT.call_once(|| {
            re_log::setup_logging();
            if cfg!(debug_assertions) {
                re_crash_handler::install_crash_handlers(re_build_info::build_info!());

                // Log a clear warning to inform users that (accidentally) use a debug build of the SDK.
                // This should however _never_ cause a panic if RERUN_PANIC_ON_WARN is set, e.g. in test environments.
                const DEBUG_BUILD_WARNING: &str =
                    "Using a DEBUG BUILD of the Rerun SDK with Rerun crash handlers!";
                let can_log_warning = std::env::var("RERUN_PANIC_ON_WARN")
                    .map(|value| value == "0")
                    .unwrap_or(true);
                if can_log_warning {
                    re_log::warn!(DEBUG_BUILD_WARNING);
                } else {
                    re_log::info!(DEBUG_BUILD_WARNING);
                }
            }
        });
    }

    let store_info = ptr::try_ptr_as_ref(store_info, "store_info")?;

    let CStoreInfo {
        application_id,
        recording_id,
        store_kind,
    } = *store_info;

    let application_id = application_id.as_nonempty_str("store_info.application_id")?;

    let mut rec_builder = RecordingStreamBuilder::new(application_id)
        //.store_id(recording_id.clone()) // TODO(andreas): Expose store id.
        .store_source(re_sdk::external::re_log_types::StoreSource::CSdk)
        .default_enabled(default_enabled);

    if let Some(recording_id) = recording_id.as_optional_str("recording_id")? {
        rec_builder = rec_builder.recording_id(recording_id);
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

#[expect(unsafe_code)]
#[unsafe(no_mangle)]
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
    pub static THREAD_LIFE_TRACKER: TrivialTypeWithDrop = const { TrivialTypeWithDrop };
}

#[expect(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn rr_recording_stream_free(id: CRecordingStream) {
    if THREAD_LIFE_TRACKER.try_with(|_v| {}).is_ok() {
        if let Some(stream) = RECORDING_STREAMS.lock().remove(id) {
            // Before we called `stream.disconnect()` here`, which unnecessarily replaced the current sink with a
            // buffered sink that would be immediately dropped afterwards. Not only did this cause spam in the
            // log outputs, it also lead to race conditions upon (log) application shutdown.
            drop(stream);
        }
    } else {
        // Yes, at least as of writing we can still log things in this state!
        re_log::debug!(
            "rr_recording_stream_free called on a thread that is shutting down and can no longer access thread locals. We can't handle this and have to ignore this call."
        );
    }
}

#[expect(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn rr_recording_stream_set_global(id: CRecordingStream, store_kind: CStoreKind) {
    let stream = RECORDING_STREAMS.lock().get(id);
    RecordingStream::set_global(store_kind.into(), stream);
}

#[expect(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn rr_recording_stream_set_thread_local(
    id: CRecordingStream,
    store_kind: CStoreKind,
) {
    let stream = RECORDING_STREAMS.lock().get(id);
    RecordingStream::set_thread_local(store_kind.into(), stream);
}

#[expect(unsafe_code)]
#[unsafe(no_mangle)]
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

#[expect(clippy::result_large_err)]
fn rr_recording_stream_is_enabled_impl(id: CRecordingStream) -> Result<bool, CError> {
    Ok(recording_stream(id)?.is_enabled())
}

#[expect(unsafe_code)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rr_recording_stream_flush_blocking(
    id: CRecordingStream,
    timeout_sec: c_float,
    error: *mut CError,
) {
    if let Some(stream) = RECORDING_STREAMS.lock().get(id) {
        let timeout = if timeout_sec.is_nan() {
            if let Some(error) = unsafe { error.as_mut() } {
                *error = CError::new(CErrorCode::InvalidTimeArgument, "NaN timeout");
            }
            Duration::ZERO
        } else if timeout_sec < 0.0 {
            if let Some(error) = unsafe { error.as_mut() } {
                *error = CError::new(CErrorCode::InvalidTimeArgument, "Negative timeout");
            }
            Duration::ZERO
        } else {
            Duration::try_from_secs_f32(timeout_sec)
                .ok()
                .unwrap_or(Duration::MAX)
        };
        if let Err(err) = stream.flush_with_timeout(timeout)
            && let Some(error) = unsafe { error.as_mut() }
        {
            let code = match &err {
                re_sdk::sink::SinkFlushError::Timeout => CErrorCode::RecordingStreamFlushTimeout,
                re_sdk::sink::SinkFlushError::Failed { .. } => {
                    CErrorCode::RecordingStreamFlushFailure
                }
            };
            *error = CError::new(code, &err.to_string());
        }
    }
}

#[expect(unsafe_code)]
#[expect(clippy::result_large_err)]
fn rr_recording_stream_set_sinks_impl(
    stream: CRecordingStream,
    raw_sinks: *mut CLogSink,
    num_sinks: u32,
) -> Result<(), CError> {
    let stream = recording_stream(stream)?;

    let raw_sinks = unsafe { std::slice::from_raw_parts_mut(raw_sinks, num_sinks as usize) };

    let mut sinks: Vec<Box<dyn re_sdk::sink::LogSink>> = Vec::with_capacity(num_sinks as usize);
    for sink in raw_sinks {
        match sink {
            CLogSink::GrpcSink { grpc } => {
                let uri = grpc
                    .url
                    .as_nonempty_str("url")?
                    .parse::<re_sdk::external::re_uri::ProxyUri>()
                    .map_err(|err| CError::new(CErrorCode::InvalidServerUrl, &err.to_string()))?;
                sinks.push(Box::new(re_sdk::sink::GrpcSink::new(uri)));
            }
            CLogSink::FileSink { file } => {
                let path = file.path.as_nonempty_str("path")?;
                sinks.push(Box::new(re_sdk::sink::FileSink::new(path).map_err(
                    |err| {
                        CError::new(
                            CErrorCode::RecordingStreamSaveFailure,
                            &format!("Failed to save recording stream to {path:?}: {err}"),
                        )
                    },
                )?));
            }
        }
    }

    stream.set_sinks(sinks);

    Ok(())
}

#[expect(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn rr_recording_stream_set_sinks(
    id: CRecordingStream,
    sinks: *mut CLogSink,
    num_sinks: u32,
    error: *mut CError,
) {
    if let Err(err) = rr_recording_stream_set_sinks_impl(id, sinks, num_sinks) {
        err.write_error(error);
    }
}

#[expect(clippy::result_large_err)]
fn rr_recording_stream_connect_grpc_impl(
    stream: CRecordingStream,
    url: CStringView,
) -> Result<(), CError> {
    let stream = recording_stream(stream)?;

    let url = url.as_nonempty_str("url")?;

    if let Err(err) = stream.connect_grpc_opts(url) {
        return Err(CError::new(CErrorCode::InvalidServerUrl, &err.to_string()));
    }

    Ok(())
}

#[expect(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn rr_recording_stream_connect_grpc(
    id: CRecordingStream,
    url: CStringView,
    error: *mut CError,
) {
    if let Err(err) = rr_recording_stream_connect_grpc_impl(id, url) {
        err.write_error(error);
    }
}

#[expect(clippy::result_large_err)]
fn rr_recording_stream_serve_grpc_impl(
    stream: CRecordingStream,
    bind_ip: CStringView,
    port: u16,
    server_memory_limit: CStringView,
    newest_first: bool,
) -> Result<(), CError> {
    let stream = recording_stream(stream)?;

    let bind_ip = bind_ip.as_nonempty_str("bind_ip")?;
    let server_options = re_sdk::ServerOptions {
        playback_behavior: re_sdk::PlaybackBehavior::from_newest_first(newest_first),

        memory_limit: server_memory_limit
            .as_maybe_empty_str("server_memory_limit")?
            .parse::<re_sdk::MemoryLimit>()
            .map_err(|err| CError::new(CErrorCode::InvalidMemoryLimit, &err))?,
    };

    stream
        .serve_grpc_opts(bind_ip, port, server_options)
        .map_err(|err| {
            CError::new(
                CErrorCode::RecordingStreamServeGrpcFailure,
                &err.to_string(),
            )
        })?;

    Ok(())
}

#[expect(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn rr_recording_stream_serve_grpc(
    id: CRecordingStream,
    bind_ip: CStringView,
    port: u16,
    server_memory_limit: CStringView,
    newest_first: bool,
    error: *mut CError,
) {
    if let Err(err) =
        rr_recording_stream_serve_grpc_impl(id, bind_ip, port, server_memory_limit, newest_first)
    {
        err.write_error(error);
    }
}

#[expect(clippy::result_large_err)]
fn rr_recording_stream_spawn_impl(
    stream: CRecordingStream,
    spawn_opts: *const CSpawnOptions,
) -> Result<(), CError> {
    let stream = recording_stream(stream)?;

    let spawn_opts = if spawn_opts.is_null() {
        re_sdk::SpawnOptions::default()
    } else {
        let spawn_opts = ptr::try_ptr_as_ref(spawn_opts, "spawn_opts")?;
        spawn_opts.as_rust()?
    };

    stream
        .spawn_opts(&spawn_opts)
        .map_err(|err| CError::new(CErrorCode::RecordingStreamSpawnFailure, &err.to_string()))?;

    Ok(())
}

#[expect(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn rr_recording_stream_spawn(
    id: CRecordingStream,
    spawn_opts: *const CSpawnOptions,
    error: *mut CError,
) {
    if let Err(err) = rr_recording_stream_spawn_impl(id, spawn_opts) {
        err.write_error(error);
    }
}

#[expect(clippy::result_large_err)]
fn rr_recording_stream_save_impl(
    stream: CRecordingStream,
    rrd_filepath: CStringView,
) -> Result<(), CError> {
    let rrd_filepath = rrd_filepath.as_nonempty_str("path")?;
    recording_stream(stream)?.save(rrd_filepath).map_err(|err| {
        CError::new(
            CErrorCode::RecordingStreamSaveFailure,
            &format!("Failed to save recording stream to {rrd_filepath:?}: {err}"),
        )
    })
}

#[expect(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn rr_recording_stream_save(
    id: CRecordingStream,
    path: CStringView,
    error: *mut CError,
) {
    if let Err(err) = rr_recording_stream_save_impl(id, path) {
        err.write_error(error);
    }
}

#[expect(clippy::result_large_err)]
fn rr_recording_stream_stdout_impl(stream: CRecordingStream) -> Result<(), CError> {
    recording_stream(stream)?.stdout().map_err(|err| {
        CError::new(
            CErrorCode::RecordingStreamStdoutFailure,
            &format!("Failed to forward recording stream to stdout: {err}"),
        )
    })
}

#[expect(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn rr_recording_stream_stdout(id: CRecordingStream, error: *mut CError) {
    if let Err(err) = rr_recording_stream_stdout_impl(id) {
        err.write_error(error);
    }
}

#[expect(clippy::result_large_err)]
fn rr_recording_stream_set_time_impl(
    stream: CRecordingStream,
    timeline_name: CStringView,
    time_type: CTimeType,
    value: i64,
) -> Result<(), CError> {
    let timeline = timeline_name.as_nonempty_str("timeline_name")?;
    let stream = recording_stream(stream)?;
    let time_type = match time_type {
        CTimeType::Sequence => TimeType::Sequence,
        CTimeType::Duration => TimeType::DurationNs,
        CTimeType::Timestamp => TimeType::TimestampNs,
    };
    stream.set_time(timeline, TimeCell::new(time_type, value));
    Ok(())
}

#[expect(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn rr_recording_stream_set_time(
    stream: CRecordingStream,
    timeline_name: CStringView,
    time_type: CTimeType,
    value: i64,
    error: *mut CError,
) {
    if let Err(err) = rr_recording_stream_set_time_impl(stream, timeline_name, time_type, value) {
        err.write_error(error);
    }
}

#[expect(clippy::result_large_err)]
fn rr_recording_stream_disable_timeline_impl(
    stream: CRecordingStream,
    timeline_name: CStringView,
) -> Result<(), CError> {
    let timeline = timeline_name.as_nonempty_str("timeline_name")?;
    recording_stream(stream)?.disable_timeline(timeline);
    Ok(())
}

#[expect(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn rr_recording_stream_disable_timeline(
    stream: CRecordingStream,
    timeline_name: CStringView,
    error: *mut CError,
) {
    if let Err(err) = rr_recording_stream_disable_timeline_impl(stream, timeline_name) {
        err.write_error(error);
    }
}

#[expect(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn rr_recording_stream_reset_time(stream: CRecordingStream) {
    if let Some(stream) = RECORDING_STREAMS.lock().get(stream) {
        stream.reset_time();
    }
}

#[expect(unsafe_code)]
#[expect(clippy::result_large_err)]
#[expect(clippy::needless_pass_by_value)] // Conceptually we're consuming the data_row, as we take ownership of data it points to.
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

    let entity_path = entity_path.as_maybe_empty_str("entity_path")?;
    let entity_path = EntityPath::parse_forgiving(entity_path);

    let num_data_cells = num_data_cells as usize;

    let batches = unsafe { std::slice::from_raw_parts_mut(batches, num_data_cells) };

    let mut components = IntMap::default();
    {
        let component_type_registry = COMPONENT_TYPES.read();

        for batch in batches {
            let CComponentBatch {
                component_type,
                array,
            } = batch;
            let component_type = component_type_registry.get(*component_type)?;
            let datatype = component_type.datatype.clone();
            let array = unsafe { FFI_ArrowArray::from_raw(array) }; // Move out from `batches`
            let values = unsafe { arrow_array_from_c_ffi(array, datatype) }?;
            let batch =
                re_sdk::SerializedComponentBatch::new(values, component_type.descriptor.clone());
            components.insert(batch.descriptor.component, batch);
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

#[expect(unsafe_code)]
#[unsafe(no_mangle)]
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

#[expect(clippy::result_large_err)]
fn rr_recording_stream_log_file_from_path_impl(
    stream: CRecordingStream,
    filepath: CStringView,
    entity_path_prefix: CStringView,
    transform_frame_prefix: CStringView,
    static_: bool,
) -> Result<(), CError> {
    let stream = recording_stream(stream)?;

    let filepath = filepath.as_nonempty_str("filepath")?;
    let entity_path_prefix = entity_path_prefix.as_optional_str("entity_path_prefix")?;
    let transform_frame_prefix =
        transform_frame_prefix.as_optional_str("transform_frame_prefix")?;

    stream
        .log_file_from_path(
            filepath,
            entity_path_prefix.map(Into::into),
            transform_frame_prefix.map(Into::into),
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

#[expect(unsafe_code)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rr_recording_stream_log_file_from_path(
    stream: CRecordingStream,
    filepath: CStringView,
    entity_path_prefix: CStringView,
    transform_frame_prefix: CStringView,
    static_: bool,
    error: *mut CError,
) {
    if let Err(err) = rr_recording_stream_log_file_from_path_impl(
        stream,
        filepath,
        entity_path_prefix,
        transform_frame_prefix,
        static_,
    ) {
        err.write_error(error);
    }
}

#[expect(clippy::result_large_err)]
fn rr_recording_stream_log_file_from_contents_impl(
    stream: CRecordingStream,
    filepath: CStringView,
    contents: CBytesView,
    entity_path_prefix: CStringView,
    transform_frame_prefix: CStringView,
    static_: bool,
) -> Result<(), CError> {
    let stream = recording_stream(stream)?;

    let filepath = filepath.as_nonempty_str("filepath")?;
    let contents = contents.as_bytes("contents")?;
    let entity_path_prefix = entity_path_prefix.as_optional_str("entity_path_prefix")?;
    let transform_frame_prefix =
        transform_frame_prefix.as_optional_str("transform_frame_prefix")?;

    stream
        .log_file_from_contents(
            filepath,
            std::borrow::Cow::Borrowed(contents),
            entity_path_prefix.map(Into::into),
            transform_frame_prefix.map(Into::into),
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

#[expect(unsafe_code)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rr_recording_stream_log_file_from_contents(
    stream: CRecordingStream,
    filepath: CStringView,
    contents: CBytesView,
    entity_path_prefix: CStringView,
    transform_frame_prefix: CStringView,
    static_: bool,
    error: *mut CError,
) {
    if let Err(err) = rr_recording_stream_log_file_from_contents_impl(
        stream,
        filepath,
        contents,
        entity_path_prefix,
        transform_frame_prefix,
        static_,
    ) {
        err.write_error(error);
    }
}

#[expect(unsafe_code)]
#[expect(clippy::result_large_err)]
fn rr_recording_stream_send_columns_impl(
    stream: CRecordingStream,
    entity_path: CStringView,
    time_columns: &mut [CTimeColumn],
    component_columns: &mut [CComponentColumns],
) -> Result<(), CError> {
    // Create chunk-id as early as possible. It has a timestamp and is used to estimate e2e latency.
    let id = ChunkId::new();

    let stream = recording_stream(stream)?;
    let entity_path = entity_path.as_maybe_empty_str("entity_path")?;

    let time_columns: IntMap<TimelineName, TimeColumn> = time_columns
        .iter_mut()
        .map(|time_column| {
            let timeline: Timeline = time_column.timeline.clone().try_into()?;
            let datatype = arrow::datatypes::DataType::Int64;
            let array = unsafe { FFI_ArrowArray::from_raw(&mut time_column.times) } ; // Move out of the array
            let time_values_untyped = unsafe { arrow_array_from_c_ffi(array, datatype) }?;
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
            .iter_mut()
            .map(|batch| {
                let CComponentColumns {
                    component_type,
                    array,
                } = batch;
                let component_type = component_type_registry.get(*component_type)?;

                let nullable = true;
                let list_datatype = arrow::datatypes::DataType::List(arrow::datatypes::Field::new_list_field(component_type.datatype.clone(), nullable).into());

                let array = unsafe { FFI_ArrowArray::from_raw(array) }; // Move out of the array
                let component_values_untyped = unsafe { arrow_array_from_c_ffi(array, list_datatype) }?;
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

#[expect(unsafe_code)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rr_recording_stream_send_columns(
    stream: CRecordingStream,
    entity_path: CStringView,
    time_columns: *mut CTimeColumn,
    num_time_columns: u32,
    component_batches: *mut CComponentColumns,
    num_component_batches: u32,
    error: *mut CError,
) {
    let time_columns =
        unsafe { std::slice::from_raw_parts_mut(time_columns, num_time_columns as usize) };
    let component_batches = unsafe {
        std::slice::from_raw_parts_mut(component_batches, num_component_batches as usize)
    };

    if let Err(err) =
        rr_recording_stream_send_columns_impl(stream, entity_path, time_columns, component_batches)
    {
        err.write_error(error);
    }
}

// ----------------------------------------------------------------------------
// Private functions

#[expect(unsafe_code)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _rr_escape_entity_path_part(part: CStringView) -> *const c_char {
    let Ok(part) = part.as_maybe_empty_str("entity_path_part") else {
        return std::ptr::null();
    };

    let part = re_sdk::EntityPathPart::from(part).escaped_string();

    let Ok(part) = CString::new(part) else {
        return std::ptr::null();
    };

    part.into_raw()
}

#[expect(unsafe_code)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _rr_free_string(str: *mut c_char) {
    if str.is_null() {
        return;
    }

    // Free the string:
    unsafe {
        // SAFETY: `_rr_free_string` should only be called on strings allocated by `_rr_escape_entity_path_part`.
        std::mem::drop(CString::from_raw(str));
    }
}
