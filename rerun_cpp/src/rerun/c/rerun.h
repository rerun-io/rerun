// ----------------------------------------------------------------------------
// The Rerun C SDK for Rerun version 0.10.0-alpha.9+dev
// ----------------------------------------------------------------------------
// This file is part of the rerun_c Rust crate.
// EDITS TO COPIES OUTSIDE OF RERUN_C WILL BE OVERWRITTEN.
// ----------------------------------------------------------------------------
//
// All Rerun functions and types are thread-safe,
// which means you can share a `rr_recording_stream` across threads.

#ifndef RERUN_H
#define RERUN_H

#ifdef __cplusplus
extern "C" {
#endif

#include <stdbool.h>
#include <stdint.h>

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
    RERUN_STORE_KIND_RECORDING = 1,
    RERUN_STORE_KIND_BLUEPRINT = 2,
};

/// Special value for `rr_recording_stream` methods to indicate the most appropriate
/// globally available recording stream for recordings.
/// (i.e. thread-local first, then global scope)
#define RERUN_REC_STREAM_CURRENT_RECORDING 0xFFFFFFFF

/// Special value for `rr_recording_stream` methods to indicate the most appropriate
/// globally available recording stream for blueprints.
/// (i.e. thread-local first, then global scope)
#define RERUN_REC_STREAM_CURRENT_BLUEPRINT 0xFFFFFFFE

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

typedef struct rr_store_info {
    /// The user-chosen name of the application doing the logging.
    rr_string application_id;

    /// `RERUN_STORE_KIND_RECORDING` or `RERUN_STORE_KIND_BLUEPRINT`
    rr_store_kind store_kind;
} rr_store_info;

/// Arrow-encoded data of a single component for a single entity.
typedef struct rr_data_cell {
    /// The name of the component, e.g. `position`.
    rr_string component_name;

    /// The number of bytes in the `bytes` field.
    /// Must be a multiple of 8.
    uint64_t num_bytes;

    /// Data in the Arrow IPC encapsulated message format.
    ///
    /// There must be exactly one chunk of data.
    ///
    /// * <https://arrow.apache.org/docs/format/Columnar.html#format-ipc>
    /// * <https://wesm.github.io/arrow-site-test/format/IPC.html#encapsulated-message-format>
    const uint8_t* bytes;
} rr_data_cell;

/// Arrow-encoded log data for a single entity.
/// May contain many components.
typedef struct {
    /// Where to log to, e.g. `world/camera`.
    rr_string entity_path;

    /// Number of instances of this entity (e.g. number of points in a point
    /// cloud).
    uint32_t num_instances;

    /// Number of components.
    uint32_t num_data_cells;

    /// One for each component.
    const rr_data_cell* data_cells;
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
    RR_ERROR_CODE_INVALID_ENTITY_PATH,

    // Recording stream errors
    _RR_ERROR_CODE_CATEGORY_RECORDING_STREAM = 0x000000100,
    RR_ERROR_CODE_RECORDING_STREAM_CREATION_FAILURE,
    RR_ERROR_CODE_RECORDING_STREAM_SAVE_FAILURE,
    RR_ERROR_CODE_RECORDING_STREAM_SPAWN_FAILURE,

    // Arrow data processing errors.
    _RR_ERROR_CODE_CATEGORY_ARROW = 0x000001000,
    RR_ERROR_CODE_ARROW_IPC_MESSAGE_PARSING_FAILURE,
    RR_ERROR_CODE_ARROW_DATA_CELL_ERROR,

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
extern const char* rr_version_string(void);

/// Spawns a new Rerun Viewer process from an executable available in PATH, ready to
/// listen for incoming TCP connections.
///
/// `spawn_opts` can be set to NULL to use the recommended defaults.
///
/// If a Rerun Viewer is already listening on this TCP port, this does nothing.
extern void rr_spawn(const rr_spawn_options* spawn_opts, rr_error* error);

/// Creates a new recording stream to log to.
///
/// You must call this at least once to enable logging.
///
/// Usually you only have one recording stream, so you can call
/// `rr_recording_stream_set_global` afterwards once to make it available globally via
/// `RERUN_REC_STREAM_CURRENT_RECORDING` and `RERUN_REC_STREAM_CURRENT_BLUEPRINT` respectively.
///
/// @return A handle to the recording stream, or null if an error occurred.
extern rr_recording_stream rr_recording_stream_new(
    const rr_store_info* store_info, bool default_enabled, rr_error* error
);

/// Free the given recording stream. The handle will be invalid after this.
///
/// Flushes the stream before freeing it, but does *not* block.
///
/// Does nothing for `RERUN_REC_STREAM_CURRENT_RECORDING` and `RERUN_REC_STREAM_CURRENT_BLUEPRINT`.
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

/// Stream all log-data to a given file.
///
/// This function returns immediately.
extern void rr_recording_stream_save(rr_recording_stream stream, rr_string path, rr_error* error);

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
/// The timeline is still there, but it will not be updated with any new data.
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
extern void rr_recording_stream_log(
    rr_recording_stream stream, const rr_data_row* data_row, bool inject_time, rr_error* error
);

// ----------------------------------------------------------------------------

#ifdef __cplusplus
}
#endif

#endif // RERUN_H
