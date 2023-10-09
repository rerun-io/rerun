#pragma once

#include <memory> // shared_ptr

namespace arrow {
    class Buffer;
}

namespace rerun {
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

    // TODO: move and document?
    struct SerializedComponentBatch {
        SerializedComponentBatch() = default;

        SerializedComponentBatch(size_t _num_instances, DataCell _data_cell)
            : num_instances(_num_instances), data_cell(std::move(_data_cell)) {}

        size_t num_instances;
        DataCell data_cell;
    };
} // namespace rerun
