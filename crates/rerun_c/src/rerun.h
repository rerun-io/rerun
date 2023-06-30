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
/// Usually you only have one recording stream, so you can call `rerun_rec_stream_new` once,
/// ignore its return value, and use `RERUN_REC_STREAM_DEFAULT` everywhere in your code.
#define RERUN_REC_STREAM_DEFAULT 0

/// A unique handle for a recording stream.
///
/// The default is RERUN_REC_STREAM_DEFAULT
typedef int32_t RerunRecStream;

struct RerunStoreInfo {
    /// The user-chosen name of the application doing the logging.
    const char*  application_id;

    /// `RERUN_STORE_KIND_RECORDING` or `RERUN_STORE_KIND_BLUEPRINT`
    int32_t      store_kind;
};

// ----------------------------------------------------------------------------
// Functions:

/// Returns a human-readable version string of the Rerun C SDK.
extern const char* rerun_version_string();

extern void rerun_print_hello_world();

/// Create a new recording stream to log to.
///
/// You must call this at least once to enable logging.
///
/// The first call always returns `RERUN_REC_STREAM_DEFAULT`.
/// Usually you only have one recording stream, so you can call `rerun_rec_stream_new` once,
/// ignore its return value, and use `RERUN_REC_STREAM_DEFAULT` everywhere in your code.
extern RerunRecStream rerun_rec_stream_new(const struct RerunStoreInfo* store_info, const char* tcp_addr);

/// Free the given recording stream. The handle will be invalid after this.
extern void rerun_rec_stream_free(RerunRecStream stream);

// ----------------------------------------------------------------------------

#ifdef __cplusplus
}
#endif

#endif // RERUN_H
