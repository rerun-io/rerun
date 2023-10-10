#pragma once

#include <memory> // shared_ptr

namespace arrow {
    class Buffer;
}

namespace rerun {
    /// Equivalent to `rr_data_cell` from the C API.
    struct DataCell {
        /// Name of the logged component.
        const char* component_name;

        /// Data in the Arrow IPC encapsulated message format.
        ///
        /// There must be exactly one chunk of data.
        ///
        /// * <https://arrow.apache.org/docs/format/Columnar.html#format-ipc>
        /// * <https://wesm.github.io/arrow-site-test/format/IPC.html#encapsulated-message-format>
        std::shared_ptr<arrow::Buffer> buffer;
    };
} // namespace rerun
