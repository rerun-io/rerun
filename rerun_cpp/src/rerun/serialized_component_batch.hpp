#pragma once

#include "data_cell.hpp"

namespace rerun {
    /// One or more instances of a single component serialized using Apache Arrow.
    struct SerializedComponentBatch {
        SerializedComponentBatch() = default;

        SerializedComponentBatch(DataCell data_cell_, size_t num_instances_)
            : data_cell(std::move(data_cell_)), num_instances(num_instances_) {}

        /// The underlying data.
        DataCell data_cell;

        /// How many components were serialized.
        size_t num_instances;
    };
} // namespace rerun
