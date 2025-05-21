// ----------------------------------------------------------------------------
// The Rerun C SDK for Rerun.
// This file is part of the rerun_c Rust crate.
// ----------------------------------------------------------------------------
//
// All Rerun functions and types are thread-safe,
// which means you can share a `rr_recording_stream` across threads.
// ----------------------------------------------------------------------------

#ifndef RERUN_H
#define RERUN_H

#ifdef __cplusplus
extern "C" {
#endif

#include <stdbool.h>
#include <stdint.h>
#include "arrow_c_data_interface.h"
#include "compiler_utils.h"
#include "sdk_info.h"

// ----------------------------------------------------------------------------
// Types:

/// A Utf8 string with a length in bytes.
typedef struct rr_string {
    /// Pointer to a UTF8 string.
    ///
    /// Does *not* need to be null-terminated.
    /// Rerun is guaranteed to not read beyond utf8[length_in_bytes-1].
    const char* utf8;

    /// The length of the string in bytes (*excluding* null-terminator, if any).
    uint32_t length_in_bytes;
} rr_string;

/// A byte slice.
typedef struct rr_bytes {
    /// Pointer to the bytes.
    ///
    /// Rerun is guaranteed to not read beyond bytes[length-1].
    const uint8_t* bytes;

    /// The length of the data in bytes.
    uint32_t length;
} rr_bytes;

#ifndef __cplusplus

#include <string.h> // For strlen

/// Create a `rr_string` from a null-terminated string.
///
/// Calling with NULL is safe.
rr_string rr_make_string(const char* utf8) {
    uint32_t length_in_bytes = 0;
    if (utf8 != NULL) {
        length_in_bytes = (uint32_t)strlen(utf8);
    }
    return (rr_string){.utf8 = utf8, .length_in_bytes = length_in_bytes};
}

#endif

/// Type of store log messages are sent to.
typedef uint32_t rr_store_kind;

enum {
    RR_STORE_KIND_RECORDING = 1,
    RR_STORE_KIND_BLUEPRINT = 2,
};

/// Special value for `rr_recording_stream` methods to indicate the most appropriate
/// globally available recording stream for recordings.
/// (i.e. thread-local first, then global scope)
#define RR_REC_STREAM_CURRENT_RECORDING 0xFFFFFFFF

/// Special value for `rr_recording_stream` methods to indicate the most appropriate
/// globally available recording stream for blueprints.
/// (i.e. thread-local first, then global scope)
#define RR_REC_STREAM_CURRENT_BLUEPRINT 0xFFFFFFFE

/// Handle to a component type that can be registered.
typedef uint32_t rr_component_type_handle;

/// Special value for `rr_component_type_handle` to indicate an invalid handle.
#define RR_COMPONENT_TYPE_HANDLE_INVALID 0xFFFFFFFF

/// A unique handle for a recording stream.
/// A recording stream handles everything related to logging data into Rerun.
///
/// ## Multithreading and ordering
///
/// Internally, all operations are linearized into a pipeline:
/// - All operations sent by a given thread will take effect in the same exact order as that
///   thread originally sent them in, from its point of view.
/// - There isn't any well defined global order across multiple threads.
///
/// This means that e.g. flushing the pipeline (`rr_recording_stream_flush_blocking`) guarantees
/// that all previous data sent by the calling thread has been recorded; no more, no less.
/// (e.g. it does not mean that all file caches are flushed)
///
/// ## Shutdown
///
/// The recording stream can only be shutdown by dropping all instances of it, at which point
/// it will automatically take care of flushing any pending data that might remain in the
/// pipeline.
///
/// TODO(andreas): The only way of having two instances of a `RecordingStream` is currently to
/// set it as a the global.
typedef uint32_t rr_recording_stream;

/// Options to control the behavior of `spawn`.
///
/// Refer to the field-level documentation for more information about each individual options.
///
/// The defaults are ok for most use cases.
typedef struct rr_spawn_options {
    /// The port to listen on.
    ///
    /// Defaults to `9876` if set to `0`.
    uint16_t port;

    /// An upper limit on how much memory the Rerun Viewer should use.
    /// When this limit is reached, Rerun will drop the oldest data.
    /// Example: `16GB` or `50%` (of system total).
    ///
    /// Defaults to `75%` if null.
    rr_string memory_limit;

    /// Hide the normal Rerun welcome screen.
    bool hide_welcome_screen;

    /// Detach Rerun Viewer process from the application process.
    bool detach_process;

    /// Specifies the name of the Rerun executable.
    ///
    /// You can omit the `.exe` suffix on Windows.
    ///
    /// Defaults to `rerun` if null.
    rr_string executable_name;

    /// Enforce a specific executable to use instead of searching though PATH
    /// for [`Self::executable_name`].
    ///
    /// Unspecified by default.
    rr_string executable_path;
} rr_spawn_options;

/// Recommended settings for the [`DataLoader`].
///
/// The loader is free to ignore some or all of these.
///
/// Refer to the field-level documentation for more information about each individual options.
//
// TODO(#3841): expose timepoint settings once we implement stateless APIs
typedef struct rr_data_loader_settings {
    /// The recommended `RecordingId` to log the data to.
    ///
    /// Unspecified by default.
    rr_string recording_id;

    /// What should the logged entity paths be prefixed with?
    ///
    /// Unspecified by default.
    rr_string entity_path_prefix;

    /// Should the logged data be static?
    ///
    /// Defaults to `false` if not set.
    bool static_;
} rr_data_loader_settings;

typedef struct rr_store_info {
    /// The user-chosen name of the application doing the logging.
    rr_string application_id;

    /// The user-chosen name of the recording being logged to.
    ///
    /// Defaults to a random ID if unspecified.
    rr_string recording_id;

    /// `RR_STORE_KIND_RECORDING` or `RR_STORE_KIND_BLUEPRINT`
    rr_store_kind store_kind;
} rr_store_info;

/// Definition of a component descriptor that can be registered.
typedef struct rr_component_descriptor {
    /// Optional name of the `Archetype` associated with this data.
    ///
    /// Null if the data wasn't logged through an archetype.
    ///
    /// Example: `rerun.archetypes.Points3D`.
    rr_string archetype_name;

    /// Optional name of the field within `Archetype` associated with this data.
    ///
    /// Null if the data wasn't logged through an archetype.
    ///
    /// Example: `positions`.
    rr_string archetype_field_name;

    /// Semantic name associated with this data.
    ///
    /// This is fully implied by `archetype_name` and `archetype_field`, but
    /// included for semantic convenience.
    ///
    /// Example: `rerun.components.Position3D`.
    rr_string component_name;
} rr_component_descriptor;

/// Definition of a component type that can be registered.
typedef struct rr_component_type {
    /// The complete descriptor for this component.
    rr_component_descriptor descriptor;

    /// The arrow schema used for arrow arrays of instances of this component.
    struct ArrowSchema schema;
} rr_component_type;

/// Arrow-encoded data of a single batch components for a single entity.
typedef struct rr_component_batch {
    /// The component type to use for this batch.
    rr_component_type_handle component_type;

    /// A batch of instances of this component serialized into an arrow array.
    struct ArrowArray array;
} rr_component_batch;

/// Arrow-encoded log data for a single entity.
/// May contain many components.
typedef struct rr_data_row {
    /// Where to log to, e.g. `world/camera`.
    rr_string entity_path;

    /// Number of different component batches.
    uint32_t num_component_batches;

    /// One for each component.
    rr_component_batch* component_batches;
} rr_data_row;

/// Arrow-encoded data of a column of components.
///
/// This is essentially an array of `rr_component_batch` with all batches
/// continuously in a single array.
typedef struct rr_component_column {
    /// The component type used for the components inside the list array.
    ///
    /// This is *not* the type of the arrow list array itself, but of the underlying batch.
    rr_component_type_handle component_type;

    /// A ListArray with the datatype `List(component_type)`.
    struct ArrowArray array;
} rr_component_column;

/// Describes whether an array is known to be sorted or not.
typedef uint32_t rr_sorting_status;

enum {
    /// It's not known whether the array is sorted or not.
    RR_SORTING_STATUS_UNKNOWN = 0,

    /// The array is known to be sorted.
    RR_SORTING_STATUS_SORTED = 1,

    /// The array is known to be unsorted.
    RR_SORTING_STATUS_UNSORTED = 2,
};

/// Describes the type of a timeline or time point.
typedef uint32_t rr_time_type;

enum {
    // 0 no longer in use

    /// Used e.g. for frames in a film.
    RR_TIME_TYPE_SEQUENCE = 1,

    /// Nanoseconds.
    RR_TIME_TYPE_DURATION = 2,

    /// Nanoseconds since Unix epoch (1970-01-01 00:00:00 UTC).
    RR_TIME_TYPE_TIMESTAMP = 3,
};

/// Definition of a timeline.
typedef struct rr_timeline {
    /// The name of the timeline.
    rr_string name;

    /// The type of the timeline.
    rr_time_type type;
} rr_timeline;

/// A column of timestamps for a given timeline.
typedef struct rr_time_column {
    /// The timeline this column belongs to.
    rr_timeline timeline;

    /// Time points as a primitive array of i64.
    struct ArrowArray array;

    /// The sorting order of the `times` array.
    rr_sorting_status sorting_status;
} rr_time_column;

/// Error codes returned by the Rerun C SDK as part of `rr_error`.
///
/// Category codes are used to group errors together, but are never returned directly.
typedef uint32_t rr_error_code;

enum {
    RR_ERROR_CODE_OK = 0,

    // Invalid argument errors.
    _RR_ERROR_CODE_CATEGORY_ARGUMENT = 0x00000010,
    RR_ERROR_CODE_UNEXPECTED_NULL_ARGUMENT,
    RR_ERROR_CODE_INVALID_STRING_ARGUMENT,
    RR_ERROR_CODE_INVALID_ENUM_VALUE,
    RR_ERROR_CODE_INVALID_RECORDING_STREAM_HANDLE,
    RR_ERROR_CODE_INVALID_SOCKET_ADDRESS,
    RR_ERROR_CODE_INVALID_COMPONENT_TYPE_HANDLE,

    // Recording stream errors
    _RR_ERROR_CODE_CATEGORY_RECORDING_STREAM = 0x00000100,
    RR_ERROR_CODE_RECORDING_STREAM_RUNTIME_FAILURE,
    RR_ERROR_CODE_RECORDING_STREAM_CREATION_FAILURE,
    RR_ERROR_CODE_RECORDING_STREAM_SAVE_FAILURE,
    RR_ERROR_CODE_RECORDING_STREAM_STDOUT_FAILURE,
    RR_ERROR_CODE_RECORDING_STREAM_SPAWN_FAILURE,
    RR_ERROR_CODE_RECORDING_STREAM_CHUNK_VALIDATION_FAILURE,

    // Arrow data processing errors.
    _RR_ERROR_CODE_CATEGORY_ARROW = 0x00001000,
    RR_ERROR_CODE_ARROW_FFI_SCHEMA_IMPORT_ERROR,
    RR_ERROR_CODE_ARROW_FFI_ARRAY_IMPORT_ERROR,

    // Utility errors.
    _RR_ERROR_CODE_CATEGORY_UTILITIES = 0x00010000,
    RR_ERROR_CODE_VIDEO_LOAD_ERROR,

    // Generic errors.
    RR_ERROR_CODE_UNKNOWN,
};

/// Error outcome object (success or error) that may be filled for fallible operations.
///
/// Passing this error struct is always optional, and you can pass `NULL` if you don't care about
/// the error in which case failure will be silent.
/// If no error occurs, the error struct will be left untouched.
typedef struct rr_error {
    /// Error code indicating the type of error.
    rr_error_code code;

    /// Human readable description of the error in null-terminated UTF8.
    //
    // NOTE: You must update `CError::MAX_MESSAGE_SIZE_BYTES` too if you modify this value.
    char description[2048];
} rr_error;

// ----------------------------------------------------------------------------
// Functions:

/// Returns a human-readable version string of the Rerun C SDK.
///
/// This should match the string in `RERUN_SDK_HEADER_VERSION`.
/// If not, the SDK's binary and the C header are out of sync.
extern const char* rr_version_string(void);

/// Spawns a new Rerun Viewer process from an executable available in PATH, ready to
/// listen for incoming gRPC connections.
///
/// `spawn_opts` can be set to NULL to use the recommended defaults.
///
/// If a Rerun Viewer is already listening on this gRPC port, this does nothing.
extern void rr_spawn(const rr_spawn_options* spawn_opts, rr_error* error);

/// Registers a new component type to be used in `rr_component_batch`.
///
/// A component with a given name can only be registered once.
/// Takes ownership of the passed arrow schema and will release it once it is no longer needed.
extern rr_component_type_handle rr_register_component_type(
    rr_component_type component_type, rr_error* error
);

/// Creates a new recording stream to log to.
///
/// You must call this at least once to enable logging.
///
/// Usually you only have one recording stream, so you can call
/// `rr_recording_stream_set_global` afterwards once to make it available globally via
/// `RR_REC_STREAM_CURRENT_RECORDING` and `RR_REC_STREAM_CURRENT_BLUEPRINT` respectively.
///
/// @return A handle to the recording stream, or null if an error occurred.
extern rr_recording_stream rr_recording_stream_new(
    const rr_store_info* store_info, bool default_enabled, rr_error* error
);

/// Free the given recording stream. The handle will be invalid after this.
///
/// Flushes the stream before freeing it, but does *not* block.
///
/// Does nothing for `RR_REC_STREAM_CURRENT_RECORDING` and `RR_REC_STREAM_CURRENT_BLUEPRINT`.
///
/// No-op for destroyed/non-existing streams.
extern void rr_recording_stream_free(rr_recording_stream stream);

/// Replaces the currently active recording of the specified type in the global scope with
/// the specified one.
extern void rr_recording_stream_set_global(rr_recording_stream stream, rr_store_kind store_kind);

/// Replaces the currently active recording of the specified type in the thread-local scope
/// with the specified one.
extern void rr_recording_stream_set_thread_local(
    rr_recording_stream stream, rr_store_kind store_kind
);

/// Check whether the recording stream is enabled.
extern bool rr_recording_stream_is_enabled(rr_recording_stream stream, rr_error* error);

/// Connect to a remote Rerun Viewer on the given URL.
///
/// Requires that you first start a Rerun Viewer by typing 'rerun' in a terminal.
///
/// url:
/// The scheme must be one of `rerun://`, `rerun+http://`, or `rerun+https://`,
/// and the pathname must be `/proxy`.
///
/// The default is `rerun+http://127.0.0.1:9876/proxy`.
///
/// flush_timeout_sec:
/// The minimum time the SDK will wait during a flush before potentially
/// dropping data if progress is not being made. Passing a negative value indicates no timeout,
/// and can cause a call to `flush` to block indefinitely.
///
/// This function returns immediately and will only raise an error for argument parsing errors,
/// not for connection errors as these happen asynchronously.
extern void rr_recording_stream_connect_grpc(
    rr_recording_stream stream, rr_string url, float flush_timeout_sec, rr_error* error
);

/// Swaps the underlying sink for a gRPC server sink pre-configured to listen on `rerun+http://{bind_ip}:{port}/proxy`.
///
/// The gRPC server will buffer all log data in memory so that late connecting viewers will get all the data.
/// You can limit the amount of data buffered by the gRPC server with the `server_memory_limit` argument.
/// Once reached, the earliest logged data will be dropped. Static data is never dropped.
extern void rr_recording_stream_serve_grpc(
    rr_recording_stream stream, rr_string bind_ip, uint16_t port, rr_string server_memory_limit,
    rr_error* error
);

/// Spawns a new Rerun Viewer process from an executable available in PATH, then connects to it
/// over gRPC.
///
/// This function returns immediately and will only raise an error for argument parsing errors,
/// not for connection errors as these happen asynchronously.
///
/// ## Parameters
///
/// spawn_opts:
/// Configuration of the spawned process.
/// Refer to `rr_spawn_options` documentation for details.
/// Passing null is valid and will result in the recommended defaults.
///
/// flush_timeout_sec:
/// The minimum time the SDK will wait during a flush before potentially
/// dropping data if progress is not being made. Passing a negative value indicates no timeout,
/// and can cause a call to `flush` to block indefinitely.
extern void rr_recording_stream_spawn(
    rr_recording_stream stream, const rr_spawn_options* spawn_opts, float flush_timeout_sec,
    rr_error* error
);

/// Stream all log-data to a given `.rrd` file.
///
/// This function returns immediately.
extern void rr_recording_stream_save(rr_recording_stream stream, rr_string path, rr_error* error);

/// Stream all log-data to stdout.
///
/// Pipe the result into the Rerun Viewer to visualize it.
///
/// If there isn't any listener at the other end of the pipe, the `RecordingStream` will
/// default back to `buffered` mode, in order not to break the user's terminal.
///
/// This function returns immediately.
extern void rr_recording_stream_stdout(rr_recording_stream stream, rr_error* error);

/// Initiates a flush the batching pipeline and waits for it to propagate.
///
/// See `rr_recording_stream` docs for ordering semantics and multithreading guarantees.
/// No-op for destroyed/non-existing streams.
extern void rr_recording_stream_flush_blocking(rr_recording_stream stream);

/// Set the current index value of the recording, for a specific timeline, for the current calling thread.
///
/// Used for all subsequent logging performed from this same thread, until the next call
/// to one of the time setting methods.
///
/// For example:
/// `rr_recording_stream_set_time_sequence(stream, "frame_nr", RR_TIME_TYPE_SEQUENCE, frame_nr, &err)`.
extern void rr_recording_stream_set_time(
    rr_recording_stream stream, rr_string timeline_name, rr_time_type time_type, int64_t value,
    rr_error* error
);

/// Stops logging to the specified timeline for subsequent log calls.
///
/// The timeline is still there, but will not be updated with any new data.
///
/// No-op if the timeline doesn't exist.
void rr_recording_stream_disable_timeline(
    rr_recording_stream stream, rr_string timeline_name, rr_error* error
);

/// Clears out the current time of the recording, for the current calling thread.
///
/// Used for all subsequent logging performed from this same thread, until the next call
/// to one of the time setting methods.
///
/// No-op for destroyed/non-existing streams.
extern void rr_recording_stream_reset_time(rr_recording_stream stream);

/// Log the given data to the given stream.
///
/// If `inject_time` is set to `true`, the row's timestamp data will be
/// overridden using the recording streams internal clock.
///
/// Takes ownership of the passed data component batches and will release underlying
/// arrow data once it is no longer needed.
/// Any pointers passed via `rr_string` can be safely freed after this call.
extern void rr_recording_stream_log(
    rr_recording_stream stream, rr_data_row data_row, bool inject_time, rr_error* error
);

/// Logs the file at the given `path` using all `DataLoader`s available.
///
/// A single `path` might be handled by more than one loader.
///
/// This method blocks until either at least one `DataLoader` starts streaming data in
/// or all of them fail.
///
/// See <https://www.rerun.io/docs/reference/data-loaders/overview> for more information.
extern void rr_recording_stream_log_file_from_path(
    rr_recording_stream stream, rr_string path, rr_string entity_path_prefix, bool static_,
    rr_error* error
);

/// Logs the given `contents` using all `DataLoader`s available.
///
/// A single `path` might be handled by more than one loader.
///
/// This method blocks until either at least one `DataLoader` starts streaming data in
/// or all of them fail.
///
/// See <https://www.rerun.io/docs/reference/data-loaders/overview> for more information.
extern void rr_recording_stream_log_file_from_contents(
    rr_recording_stream stream, rr_string path, rr_bytes contents, rr_string entity_path_prefix,
    bool static_, rr_error* error
);

/// Sends the columns of components to the stream.
///
/// Unlike the regular `log` API, which is row-oriented, this API lets you submit the data
/// in a columnar form. The lengths of all `time_columns` and `component_columns`
/// must match. All data that occurs at the same index across the different time and components
/// arrays will act as a single logical row.
///
/// Note that this API ignores any stateful time set on the log stream via the
/// `rr_recording_stream_set_time_sequence`/`rr_recording_stream_set_time_nanos`/etc. APIs.
/// Furthermore, this will _not_ inject the default timelines `log_tick` and `log_time` timeline columns.
///
/// The contents of `time_columns` and `component_columns` AFTER this call is undefined.
extern void rr_recording_stream_send_columns(
    rr_recording_stream stream, rr_string entity_path,                      //
    rr_time_column* time_columns, uint32_t num_time_columns,                //
    rr_component_column* component_columns, uint32_t num_component_columns, //
    rr_error* error
);

// ----------------------------------------------------------------------------
// Other utilities

/// Allocation method for `rr_video_asset_read_frame_timestamps_nanos`.
typedef int64_t* (*rr_alloc_timestamps)(void* alloc_context, uint32_t num_timestamps);

/// Determines the presentation timestamps of all frames inside the video.
///
/// Returned timestamps are in nanoseconds since start and are guaranteed to be monotonically increasing.
///
/// \param media_type
/// If not specified (null or empty string), the media type will be guessed from the data.
/// \param alloc_func
/// Function used to allocate memory for the returned timestamps.
/// Guaranteed to be called exactly once with the `alloc_context` pointer as argument.
extern int64_t* rr_video_asset_read_frame_timestamps_nanos(
    const uint8_t* video_bytes, uint64_t video_bytes_len, rr_string media_type, void* alloc_context,
    rr_alloc_timestamps alloc_timestamps, rr_error* error
);

// ----------------------------------------------------------------------------
// Private functions

/// PRIVATE FUNCTION: do not use.
///
/// Escape a single part of an entity path, returning an new null-terminated string.
///
/// The returned string must be freed with `_rr_free_string`.
///
/// Returns `nullptr` on failure (e.g. invalid UTF8, ore null bytes in the string).
extern char* _rr_escape_entity_path_part(rr_string part);

/// PRIVATE FUNCTION: do not use.
///
/// Must only be called with the results from `_rr_escape_entity_path_part`.
extern void _rr_free_string(char* string);

// ----------------------------------------------------------------------------

#ifdef __cplusplus
}
#endif

#endif // RERUN_H
