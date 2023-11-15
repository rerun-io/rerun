#pragma once

#include "data_cell.hpp"

namespace rerun {
    /// A `Collection` serialized using Apache Arrow.
    struct SerializedComponentBatch {
        SerializedComponentBatch() = default;

        SerializedComponentBatch(size_t _num_instances, DataCell _data_cell)
            : num_instances(_num_instances), data_cell(std::move(_data_cell)) {}

        /// How many components were serialized.
        size_t num_instances;

        /// The underlying data.
        DataCell data_cell;
    };
} // namespace rerun
