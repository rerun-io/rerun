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

/// What is returned by the first call to `rerun_rec_stream_new`.
/// Usually you only have one recording stream, so you can call
/// `rerun_rec_stream_new` once, ignore its return value, and use
/// `RERUN_REC_STREAM_DEFAULT` everywhere in your code.
#define RERUN_REC_STREAM_DEFAULT 0

/// A unique handle for a recording stream.
///
/// The default is RERUN_REC_STREAM_DEFAULT
typedef int32_t RerunRecStream;

struct RerunStoreInfo {
  /// The user-chosen name of the application doing the logging.
  const char *application_id;

  /// `RERUN_STORE_KIND_RECORDING` or `RERUN_STORE_KIND_BLUEPRINT`
  int32_t store_kind;
};

/// Arrow-encoded data of a single component for a single entity.
struct RerunDataCell {
  const char *component_name;

  /// The number of bytes in the `bytes` field.
  /// Must be a multiple of 8.
  const uint64_t num_bytes;

  /// Data in the Arrow IPC encapsulated message format.
  ///
  /// There must be exactly one chunk of data.
  ///
  /// * <https://arrow.apache.org/docs/format/Columnar.html#format-ipc>
  /// * <https://wesm.github.io/arrow-site-test/format/IPC.html#encapsulated-message-format>
  const uint8_t *bytes;
};

/// Arrow-encoded log data for a single entity.
/// May contain many components.
struct RerunDataRow {
  const char *entity_path; // Where to log to, e.g. `world/camera`.
  uint32_t num_instances;  // Number of instances of this entity (e.g. number of
                           // points in a point cloud).
  uint32_t num_data_cells; // Number of components.
  const struct RerunDataCell *data_cells; // One for each component.
};

// ----------------------------------------------------------------------------
// Functions:

/// Returns a human-readable version string of the Rerun C SDK.
extern const char *rerun_version_string(void);

extern void rerun_print_hello_world(void);

/// Create a new recording stream to log to.
///
/// You must call this at least once to enable logging.
///
/// The first call always returns `RERUN_REC_STREAM_DEFAULT`.
/// Usually you only have one recording stream, so you can call
/// `rerun_rec_stream_new` once, ignore its return value, and use
/// `RERUN_REC_STREAM_DEFAULT` everywhere in your code.
extern RerunRecStream
rerun_rec_stream_new(const struct RerunStoreInfo *store_info,
                     const char *tcp_addr);

/// Free the given recording stream. The handle will be invalid after this.
extern void rerun_rec_stream_free(RerunRecStream stream);

/// Log the given data to the given stream.
///
/// If `inject_time` is set to `true`, the row's timestamp data will be
/// overridden using the recording streams internal clock.
extern void rerun_log(RerunRecStream stream,
                      const struct RerunDataRow *data_row);

// ----------------------------------------------------------------------------

#ifdef __cplusplus
}
#endif

#endif // RERUN_H
