#pragma once

#include <memory> // shared_ptr
#include <string>
#include "result.hpp"

// TODO: That's silly.
#include "c/rerun.h"

namespace arrow {
    class Buffer;
    class Array;
    class DataType;
} // namespace arrow

struct rr_data_cell;

namespace rerun {
    /// Equivalent to `rr_data_cell` from the C API.
    struct DataCell {
        // TODO: don't alloc every time. Cache together with datatype.
        std::string component_name;
        std::shared_ptr<arrow::Array> array;
        // TODO: don't alloc every time.
        std::shared_ptr<arrow::DataType> datatype;

        /// Create a new data cell from an arrow array.
        /// TODO: silly method.
        static Result<DataCell> create(
            std::string name, const std::shared_ptr<arrow::DataType>& datatype,
            std::shared_ptr<arrow::Array> array
        );

        /// Create a data cell for an indicator component.
        static Result<rerun::DataCell> create_indicator_component(std::string arch_name);

        /// To rerun C API data cell.
        ///
        /// Only valid as long as the data cell is alive.
        Error to_c(rr_data_cell& out_cell) const;
    };
} // namespace rerun
