#pragma once

#include <initializer_list>
#include <optional>

#include "component_batch.hpp"
#include "component_column.hpp"

// Included by generated public headers; keep Arrow includes out of this file.
namespace rerun::detail {
    /// Borrows an archetype's optional component fields without copying the batches.
    using BatchFields = std::initializer_list<const std::optional<ComponentBatch>*>;

    /// Collects an archetype's set fields into batches in field order, sharing their arrays.
    Result<Collection<ComponentBatch>> collect_batches(BatchFields fields);

    /// Splits each set field into a column whose rows contain the specified numbers of values.
    Result<Collection<ComponentColumn>> partition_batches(
        BatchFields fields, const Collection<uint32_t>& lengths
    );

    /// Splits each set field into single-value rows, using the first set field's length.
    Result<Collection<ComponentColumn>> partition_batches_of_unit_length(BatchFields fields);
} // namespace rerun::detail
