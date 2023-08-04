// Arrow integrations.
#pragma once

#include <arrow/api.h>

namespace rerun {
    /// Encode the given arrow table in the Arrow IPC encapsulated message format.
    ///
    /// * <https://arrow.apache.org/docs/format/Columnar.html#format-ipc>
    /// * <https://wesm.github.io/arrow-site-test/format/IPC.html#encapsulated-message-format>
    arrow::Result<std::shared_ptr<arrow::Buffer>> ipc_from_table(const arrow::Table& table);
} // namespace rerun
