// Arrow integrations.
#pragma once

#include <memory>
#include "result.hpp"

namespace arrow {
    class Buffer;
    class Table;
} // namespace arrow

namespace rerun {
    /// Encode the given arrow table in the Arrow IPC encapsulated message format.
    ///
    /// * <https://arrow.apache.org/docs/format/Columnar.html#format-ipc>
    /// * <https://wesm.github.io/arrow-site-test/format/IPC.html#encapsulated-message-format>
    Result<std::shared_ptr<arrow::Buffer>> ipc_from_table(const arrow::Table& table);
} // namespace rerun
