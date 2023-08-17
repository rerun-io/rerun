// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.
// Based on "crates/re_types/definitions/rerun/testing/components/fuzzy.fbs"

#include "affix_fuzzer5.hpp"

#include "../arrow.hpp"
#include "../datatypes/affix_fuzzer1.hpp"

#include <arrow/api.h>

namespace rerun {
    namespace components {
        const char* AffixFuzzer5::NAME = "rerun.testing.components.AffixFuzzer5";

        const std::shared_ptr<arrow::DataType>& AffixFuzzer5::to_arrow_datatype() {
            static const auto datatype = rerun::datatypes::AffixFuzzer1::to_arrow_datatype();
            return datatype;
        }

        arrow::Result<std::shared_ptr<arrow::StructBuilder>> AffixFuzzer5::new_arrow_array_builder(
            arrow::MemoryPool* memory_pool
        ) {
            if (!memory_pool) {
                return arrow::Status::Invalid("Memory pool is null.");
            }

            return arrow::Result(
                rerun::datatypes::AffixFuzzer1::new_arrow_array_builder(memory_pool).ValueOrDie()
            );
        }

        arrow::Status AffixFuzzer5::fill_arrow_array_builder(
            arrow::StructBuilder* builder, const AffixFuzzer5* elements, size_t num_elements
        ) {
            if (!builder) {
                return arrow::Status::Invalid("Passed array builder is null.");
            }
            if (!elements) {
                return arrow::Status::Invalid("Cannot serialize null pointer to arrow array.");
            }

            (void)num_elements;
            return arrow::Status::NotImplemented(("TODO(andreas) Handle nullable extensions"));

            return arrow::Status::OK();
        }

        Result<rerun::DataCell> AffixFuzzer5::to_data_cell(
            const AffixFuzzer5* instances, size_t num_instances
        ) {
            // TODO(andreas): Allow configuring the memory pool.
            arrow::MemoryPool* pool = arrow::default_memory_pool();

            ARROW_ASSIGN_OR_RAISE(auto builder, AffixFuzzer5::new_arrow_array_builder(pool));
            if (instances && num_instances > 0) {
                ARROW_RETURN_NOT_OK(
                    AffixFuzzer5::fill_arrow_array_builder(builder.get(), instances, num_instances)
                );
            }
            std::shared_ptr<arrow::Array> array;
            ARROW_RETURN_NOT_OK(builder->Finish(&array));

            auto schema = arrow::schema(
                {arrow::field(AffixFuzzer5::NAME, AffixFuzzer5::to_arrow_datatype(), false)}
            );

            rerun::DataCell cell;
            cell.component_name = AffixFuzzer5::NAME;
            const auto result = rerun::ipc_from_table(*arrow::Table::Make(schema, {array}));
            if (result.is_err()) {
                return result.error;
            }
            cell.buffer = std::move(result.value);

            return cell;
        }
    } // namespace components
} // namespace rerun
