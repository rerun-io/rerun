#pragma once

#include <memory> // shared_ptr
#include "result.hpp"

namespace arrow {
    class Buffer;
    class Array;
    class DataType;
} // namespace arrow

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

        /// Create a new data cell from an arrow array.
        static Result<DataCell> create(
            const char* name, const std::shared_ptr<arrow::DataType>& datatype,
            std::shared_ptr<arrow::Array> array
        );

        /// Create a data cell for an indicator component.
        static Result<rerun::DataCell> create_indicator_component(const char* arch_name);
    };
} // namespace rerun
