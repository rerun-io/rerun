// The Rerun C SDK.

#ifndef RERUN_H
#define RERUN_H

#ifdef __cplusplus
extern "C" {
#endif

#include <stdint.h>

// ----------------------------------------------------------------------------
// Types:

#define RERUN_STORE_KIND_RECORDING 1
#define RERUN_STORE_KIND_BLUEPRINT 2

/// What is returned by the first call to `rr_recording_stream_new`.
/// Usually you only have one recording stream, so you can call
/// `rr_recording_stream_new` once, ignore its return value, and use
/// `RERUN_REC_STREAM_DEFAULT` everywhere in your code.
#define RERUN_REC_STREAM_DEFAULT 0

/// A unique handle for a recording stream.
///
/// The default is RERUN_REC_STREAM_DEFAULT
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
    const uint64_t num_bytes;

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

/// Create a new recording stream to log to.
///
/// You must call this at least once to enable logging.
///
/// The first call always returns `RERUN_REC_STREAM_DEFAULT`.
/// Usually you only have one recording stream, so you can call
/// `rr_recording_stream_new` once, ignore its return value, and use
/// `RERUN_REC_STREAM_DEFAULT` everywhere in your code.
extern rr_recording_stream rr_recording_stream_new(const struct rr_store_info* store_info,
                                                   const char* tcp_addr);

/// Free the given recording stream. The handle will be invalid after this.
extern void rr_recording_stream_free(rr_recording_stream stream);

/// Log the given data to the given stream.
///
/// If `inject_time` is set to `true`, the row's timestamp data will be
/// overridden using the recording streams internal clock.
extern void rr_log(rr_recording_stream stream, const struct rr_data_row* data_row);

// ----------------------------------------------------------------------------

#ifdef __cplusplus
}
#endif

#endif // RERUN_H
