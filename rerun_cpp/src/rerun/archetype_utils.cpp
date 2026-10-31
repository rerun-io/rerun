#include "archetype_utils.hpp"

#include "arrow_utils.hpp"

namespace rerun::detail {
    Result<Collection<ComponentBatch>> collect_batches(BatchFields fields) {
        std::vector<ComponentBatch> batches;
        batches.reserve(fields.size());

        for (const auto* field : fields) {
            if (field->has_value()) {
                batches.push_back(field->value());
            }
        }

        return take_ownership(std::move(batches));
    }

    namespace {
        /// Creates columns in field order, sharing one offsets buffer without consuming the batches.
        /// Absent fields are skipped; present empty batches are retained.
        Result<Collection<ComponentColumn>> partition_with_offsets(
            BatchFields fields, const std::shared_ptr<arrow::Buffer>& offsets, size_t num_rows
        ) {
            std::vector<ComponentColumn> columns;
            columns.reserve(fields.size());
            for (const auto* field : fields) {
                if (field->has_value()) {
                    auto column =
                        ComponentColumn::from_batch_with_offsets(field->value(), offsets, num_rows);
                    RR_RETURN_NOT_OK(column.error);
                    columns.push_back(std::move(column.value));
                }
            }
            return take_ownership(std::move(columns));
        }
    } // namespace

    Result<Collection<ComponentColumn>> partition_batches(
        BatchFields fields, const Collection<uint32_t>& lengths
    ) {
        for (const auto* field : fields) {
            if (field->has_value()) {
                return partition_with_offsets(
                    fields,
                    offsets_from_lengths(lengths),
                    lengths.size()
                );
            }
        }

        return Collection<ComponentColumn>();
    }

    Result<Collection<ComponentColumn>> partition_batches_of_unit_length(BatchFields fields) {
        for (const auto* field : fields) {
            if (field->has_value()) {
                const auto num_rows = field->value().length();
                std::vector<uint32_t> offsets(num_rows + 1);
                for (size_t i = 0; i < offsets.size(); ++i) {
                    offsets[i] = static_cast<uint32_t>(i);
                }
                return partition_with_offsets(
                    fields,
                    arrow_buffer_from_vector(std::move(offsets)),
                    num_rows
                );
            }
        }

        return Collection<ComponentColumn>();
    }
} // namespace rerun::detail
