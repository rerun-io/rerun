// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.
// Based on "crates/re_types/definitions/rerun/testing/components/fuzzy.fbs"

#include "affix_fuzzer9.hpp"

#include "../arrow.hpp"

#include <arrow/api.h>

namespace rerun {
    namespace components {
        const char* AffixFuzzer9::NAME = "rerun.testing.components.AffixFuzzer9";

        const std::shared_ptr<arrow::DataType>& AffixFuzzer9::to_arrow_datatype() {
            static const auto datatype = arrow::utf8();
            return datatype;
        }

        arrow::Result<std::shared_ptr<arrow::StringBuilder>> AffixFuzzer9::new_arrow_array_builder(
            arrow::MemoryPool* memory_pool
        ) {
            if (!memory_pool) {
                return arrow::Status::Invalid("Memory pool is null.");
            }

            return arrow::Result(std::make_shared<arrow::StringBuilder>(memory_pool));
        }

        arrow::Status AffixFuzzer9::fill_arrow_array_builder(
            arrow::StringBuilder* builder, const AffixFuzzer9* elements, size_t num_elements
        ) {
            if (!builder) {
                return arrow::Status::Invalid("Passed array builder is null.");
            }
            if (!elements) {
                return arrow::Status::Invalid("Cannot serialize null pointer to arrow array.");
            }

            ARROW_RETURN_NOT_OK(builder->Reserve(static_cast<int64_t>(num_elements)));
            for (size_t elem_idx = 0; elem_idx < num_elements; elem_idx += 1) {
                ARROW_RETURN_NOT_OK(builder->Append(elements[elem_idx].single_string_required));
            }

            return arrow::Status::OK();
        }

        Result<rerun::DataCell> AffixFuzzer9::to_data_cell(
            const AffixFuzzer9* instances, size_t num_instances
        ) {
            // TODO(andreas): Allow configuring the memory pool.
            arrow::MemoryPool* pool = arrow::default_memory_pool();

            ARROW_ASSIGN_OR_RAISE(auto builder, AffixFuzzer9::new_arrow_array_builder(pool));
            if (instances && num_instances > 0) {
                ARROW_RETURN_NOT_OK(
                    AffixFuzzer9::fill_arrow_array_builder(builder.get(), instances, num_instances)
                );
            }
            std::shared_ptr<arrow::Array> array;
            ARROW_RETURN_NOT_OK(builder->Finish(&array));

            auto schema = arrow::schema(
                {arrow::field(AffixFuzzer9::NAME, AffixFuzzer9::to_arrow_datatype(), false)}
            );

            rerun::DataCell cell;
            cell.component_name = AffixFuzzer9::NAME;
            const auto result = rerun::ipc_from_table(*arrow::Table::Make(schema, {array}));
            if (result.is_err()) {
                return result.error;
            }
            cell.buffer = std::move(result.value);

            return cell;
        }
    } // namespace components
} // namespace rerun
