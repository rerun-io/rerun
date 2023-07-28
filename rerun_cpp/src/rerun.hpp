// The Rerun C++ SDK.
#pragma once

// Auto-generated:
#include "archetypes.hpp"
#include "components.hpp"
#include "datatypes.hpp"

// Manually written:
#include "recording_stream.hpp"

namespace rr {
    /// The Rerun C++ SDK version as a human-readable string.
    const char* version_string();
} // namespace rr

// ----------------------------------------------------------------------------
// Arrow integration

#include <arrow/api.h>

namespace rr {
    /// Encode the given arrow table in the Arrow IPC encapsulated message format.
    ///
    /// * <https://arrow.apache.org/docs/format/Columnar.html#format-ipc>
    /// * <https://wesm.github.io/arrow-site-test/format/IPC.html#encapsulated-message-format>
    arrow::Result<std::shared_ptr<arrow::Buffer>> ipc_from_table(const arrow::Table& table);
} // namespace rr

// ----------------------------------------------------------------------------
