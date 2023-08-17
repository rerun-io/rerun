// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.
// Based on "crates/re_types/definitions/rerun/testing/components/fuzzy.fbs"

#include "affix_fuzzer2.hpp"

#include "../arrow.hpp"
#include "../datatypes/affix_fuzzer1.hpp"

#include <arrow/api.h>

namespace rerun {
    namespace components {
        const char *AffixFuzzer2::NAME = "rerun.testing.components.AffixFuzzer2";

        const std::shared_ptr<arrow::DataType> &AffixFuzzer2::to_arrow_datatype() {
            static const auto datatype = rerun::datatypes::AffixFuzzer1::to_arrow_datatype();
            return datatype;
        }

        arrow::Result<std::shared_ptr<arrow::StructBuilder>> AffixFuzzer2::new_arrow_array_builder(
            arrow::MemoryPool *memory_pool
        ) {
            if (!memory_pool) {
                return arrow::Status::Invalid("Memory pool is null.");
            }

            return arrow::Result(
                rerun::datatypes::AffixFuzzer1::new_arrow_array_builder(memory_pool).ValueOrDie()
            );
        }

        arrow::Status AffixFuzzer2::fill_arrow_array_builder(
            arrow::StructBuilder *builder, const AffixFuzzer2 *elements, size_t num_elements
        ) {
            if (!builder) {
                return arrow::Status::Invalid("Passed array builder is null.");
            }
            if (!elements) {
                return arrow::Status::Invalid("Cannot serialize null pointer to arrow array.");
            }

            static_assert(sizeof(rerun::datatypes::AffixFuzzer1) == sizeof(AffixFuzzer2));
            ARROW_RETURN_NOT_OK(rerun::datatypes::AffixFuzzer1::fill_arrow_array_builder(
                builder,
                reinterpret_cast<const rerun::datatypes::AffixFuzzer1 *>(elements),
                num_elements
            ));

            return arrow::Status::OK();
        }

        Result<rerun::DataCell> AffixFuzzer2::to_data_cell(
            const AffixFuzzer2 *instances, size_t num_instances
        ) {
            // TODO(andreas): Allow configuring the memory pool.
            arrow::MemoryPool *pool = arrow::default_memory_pool();

            ARROW_ASSIGN_OR_RAISE(auto builder, AffixFuzzer2::new_arrow_array_builder(pool));
            if (instances && num_instances > 0) {
                ARROW_RETURN_NOT_OK(
                    AffixFuzzer2::fill_arrow_array_builder(builder.get(), instances, num_instances)
                );
            }
            std::shared_ptr<arrow::Array> array;
            ARROW_RETURN_NOT_OK(builder->Finish(&array));

            auto schema = arrow::schema(
                {arrow::field(AffixFuzzer2::NAME, AffixFuzzer2::to_arrow_datatype(), false)}
            );

            rerun::DataCell cell;
            cell.component_name = AffixFuzzer2::NAME;
            const auto result = rerun::ipc_from_table(*arrow::Table::Make(schema, {array}));
            if (result.is_err()) {
                return result.error;
            }
            cell.buffer = std::move(result.value);

            return cell;
        }
    } // namespace components
} // namespace rerun
