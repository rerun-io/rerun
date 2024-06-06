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

/// Definition of a component type that can be registered.
typedef struct rr_component_type {
    /// The name of the component, e.g. `position`.
    rr_string name;

    /// The arrow schema used for arrow arrays of instances of this component.
    struct ArrowSchema schema;
} rr_component_type;

/// Arrow-encoded data of a single batch components for a single entity.
typedef struct rr_data_cell {
    /// The component type to use for this data cell.
    rr_component_type_handle component_type;

    /// A batch of instances of this component serialized into an arrow array.
    struct ArrowArray array;
} rr_data_cell;

/// Arrow-encoded log data for a single entity.
/// May contain many components.
typedef struct {
    /// Where to log to, e.g. `world/camera`.
    rr_string entity_path;

    /// Number of components.
    uint32_t num_data_cells;

    /// One for each component.
    rr_data_cell* data_cells;
} rr_data_row;

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
    RR_ERROR_CODE_INVALID_RECORDING_STREAM_HANDLE,
    RR_ERROR_CODE_INVALID_SOCKET_ADDRESS,
    RR_ERROR_CODE_INVALID_COMPONENT_TYPE_HANDLE,

    // Recording stream errors
    _RR_ERROR_CODE_CATEGORY_RECORDING_STREAM = 0x000000100,
    RR_ERROR_CODE_RECORDING_STREAM_CREATION_FAILURE,
    RR_ERROR_CODE_RECORDING_STREAM_SAVE_FAILURE,
    RR_ERROR_CODE_RECORDING_STREAM_STDOUT_FAILURE,
    RR_ERROR_CODE_RECORDING_STREAM_SPAWN_FAILURE,

    // Arrow data processing errors.
    _RR_ERROR_CODE_CATEGORY_ARROW = 0x000001000,
    RR_ERROR_CODE_ARROW_FFI_SCHEMA_IMPORT_ERROR,
    RR_ERROR_CODE_ARROW_FFI_ARRAY_IMPORT_ERROR,

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

/// Returns the version of the Rerun C SDK.
///
/// This should match the string returned by `rr_version_string`.
/// If not, the SDK's binary and the C header are out of sync.
#define RERUN_SDK_HEADER_VERSION "0.17.0-alpha.3"

/// Returns a human-readable version string of the Rerun C SDK.
///
/// This should match the string in `RERUN_SDK_HEADER_VERSION`.
/// If not, the SDK's binary and the C header are out of sync.
extern const char* rr_version_string(void);

/// Spawns a new Rerun Viewer process from an executable available in PATH, ready to
/// listen for incoming TCP connections.
///
/// `spawn_opts` can be set to NULL to use the recommended defaults.
///
/// If a Rerun Viewer is already listening on this TCP port, this does nothing.
extern void rr_spawn(const rr_spawn_options* spawn_opts, rr_error* error);

/// Registers a new component type to be used in `rr_data_cell`.
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

/// Connect to a remote Rerun Viewer on the given ip:port.
///
/// Requires that you first start a Rerun Viewer by typing 'rerun' in a terminal.
///
/// flush_timeout_sec:
/// The minimum time the SDK will wait during a flush before potentially
/// dropping data if progress is not being made. Passing a negative value indicates no timeout,
/// and can cause a call to `flush` to block indefinitely.
///
/// This function returns immediately and will only raise an error for argument parsing errors,
/// not for connection errors as these happen asynchronously.
extern void rr_recording_stream_connect(
    rr_recording_stream stream, rr_string tcp_addr, float flush_timeout_sec, rr_error* error
);

/// Spawns a new Rerun Viewer process from an executable available in PATH, then connects to it
/// over TCP.
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

/// Set the current time of the recording, for the current calling thread.
///
/// Used for all subsequent logging performed from this same thread, until the next call
/// to one of the time setting methods.
///
/// For example:
/// `rr_recording_stream_set_time_sequence(stream, "frame_nr", &frame_nr, &err)`.
extern void rr_recording_stream_set_time_sequence(
    rr_recording_stream stream, rr_string timeline_name, int64_t sequence, rr_error* error
);

/// Set the current time of the recording, for the current calling thread.
///
/// Used for all subsequent logging performed from this same thread, until the next call
/// to one of the time setting methods.
///
/// For example:
/// `rr_recording_stream_set_time_seconds(stream, "sim_time", sim_time_secs, &err)`.
extern void rr_recording_stream_set_time_seconds(
    rr_recording_stream stream, rr_string timeline_name, double seconds, rr_error* error
);

/// Set the current time of the recording, for the current calling thread.
///
/// Used for all subsequent logging performed from this same thread, until the next call
/// to one of the time setting methods.
///
/// For example:
/// `rr_recording_stream_set_time_nanos(stream, "sim_time", sim_time_nanos, &err)`.
extern void rr_recording_stream_set_time_nanos(
    rr_recording_stream stream, rr_string timeline_name, int64_t ns, rr_error* error
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
/// Takes ownership of the passed data cells and will release underlying
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
