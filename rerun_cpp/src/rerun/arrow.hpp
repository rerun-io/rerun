// Arrow integrations.
#pragma once

#include <memory>
#include "data_cell.hpp"
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

    Result<rerun::DataCell> create_indicator_component(const char* arch_name, size_t num_instances);
} // namespace rerun
