// The Rerun C++ SDK.
#pragma once

#include "recording_stream.hpp"

namespace rr {
    /// The Rerun C++ SDK version as a human-readable string.
    const char* version_string();
} // namespace rr

// ----------------------------------------------------------------------------
// Arrow integration

#include <arrow/api.h>

namespace rr {
    arrow::Result<std::shared_ptr<arrow::Table>> points3(size_t num_points, const float* xyz);

    /// Encode the given arrow table in the Arrow IPC encapsulated message format.
    ///
    /// * <https://arrow.apache.org/docs/format/Columnar.html#format-ipc>
    /// * <https://wesm.github.io/arrow-site-test/format/IPC.html#encapsulated-message-format>
    arrow::Result<std::shared_ptr<arrow::Buffer>> ipc_from_table(const arrow::Table& table);
} // namespace rr

// ----------------------------------------------------------------------------
