// The Rerun C SDK.

#ifndef RERUN_H
#define RERUN_H

#ifdef __cplusplus
extern "C" {
#endif

#include <stdbool.h>
#include <stdint.h>

// ----------------------------------------------------------------------------
// Types:

#define RERUN_STORE_KIND_RECORDING 1
#define RERUN_STORE_KIND_BLUEPRINT 2

/// Special value for `rr_recording_stream` methods to indicate the most appropriate
/// globally available recording stream for recordings.
/// (i.e. thread-local first, then global scope)
#define RERUN_REC_STREAM_CURRENT_RECORDING ((rr_recording_stream)0xFFFFFFFF)

/// Special value for `rr_recording_stream` methods to indicate the most appropriate
/// globally available recording stream for blueprints.
/// (i.e. thread-local first, then global scope)
#define RERUN_REC_STREAM_CURRENT_BLUEPRINT ((rr_recording_stream)0xFFFFFFFE)

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
typedef int32_t rr_recording_stream;

struct rr_store_info {
    /// The user-chosen name of the application doing the logging.
    const char* application_id;

    /// `RERUN_STORE_KIND_RECORDING` or `RERUN_STORE_KIND_BLUEPRINT`
    int32_t store_kind;
};

/// Arrow-encoded data of a single component for a single entity.
struct rr_data_cell {
    const char* component_name;

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
};

/// Arrow-encoded log data for a single entity.
/// May contain many components.
struct rr_data_row {
    /// Where to log to, e.g. `world/camera`.
    const char* entity_path;

    /// Number of instances of this entity (e.g. number of points in a point
    /// cloud).
    uint32_t num_instances;

    /// Number of components.
    uint32_t num_data_cells;

    /// One for each component.
    const struct rr_data_cell* data_cells;
};

// ----------------------------------------------------------------------------
// Functions:

/// Returns a human-readable version string of the Rerun C SDK.
extern const char* rr_version_string(void);

/// Creates a new recording stream to log to.
///
/// You must call this at least once to enable logging.
///
/// Usually you only have one recording stream, so you can call
/// `rr_recording_stream_set_global` afterwards once to make it available globally via
/// `RERUN_REC_STREAM_CURRENT_RECORDING` and `RERUN_REC_STREAM_CURRENT_BLUEPRINT` respectively.
extern rr_recording_stream rr_recording_stream_new(const struct rr_store_info* store_info);

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
extern void rr_recording_stream_set_global(rr_recording_stream stream, int32_t store_kind);

/// Replaces the currently active recording of the specified type in the thread-local scope
/// with the specified one.
extern void rr_recording_stream_set_thread_local(rr_recording_stream stream, int32_t store_kind);

/// Connect to a remote Rerun Viewer on the given ip:port.
///
/// Requires that you first start a Rerun Viewer by typing 'rerun' in a terminal.
///
/// flush_timeout_sec:
/// The minimum time the SDK will wait during a flush before potentially
/// dropping data if progress is not being made. Passing a negative value indicates no timeout,
/// and can cause a call to `flush` to block indefinitely.
///
/// This function returns immediately.
/// No-op for destroyed/non-existing streams.
extern void rr_recording_stream_connect(
    rr_recording_stream stream, const char* tcp_addr, float flush_timeout_sec
);

/// Stream all log-data to a given file.
///
/// This function returns immediately.
/// No-op for destroyed/non-existing streams.
extern void rr_recording_stream_save(rr_recording_stream stream, const char* path);

/// Initiates a flush the batching pipeline and waits for it to propagate.
///
/// See `rr_recording_stream` docs for ordering semantics and multithreading guarantees.
/// No-op for destroyed/non-existing streams.
extern void rr_recording_stream_flush_blocking(rr_recording_stream stream);

/// Log the given data to the given stream.
///
/// If `inject_time` is set to `true`, the row's timestamp data will be
/// overridden using the recording streams internal clock.
///
/// No-op for destroyed/non-existing streams.
extern void rr_log(
    rr_recording_stream stream, const struct rr_data_row* data_row, bool inject_time
);

// ----------------------------------------------------------------------------

#ifdef __cplusplus
}
#endif

#endif // RERUN_H
