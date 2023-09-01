// DO NOT EDIT!: This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs:54.
// Based on "crates/re_types/definitions/rerun/testing/components/fuzzy.fbs".

#include "affix_fuzzer12.hpp"

#include <arrow/builder.h>
#include <arrow/table.h>
#include <arrow/type_fwd.h>
#include <rerun/arrow.hpp>

namespace rerun {
    namespace components {
        const char *AffixFuzzer12::NAME = "rerun.testing.components.AffixFuzzer12";

        const std::shared_ptr<arrow::DataType> &AffixFuzzer12::arrow_datatype() {
            static const auto datatype = arrow::list(arrow::field("item", arrow::utf8(), false));
            return datatype;
        }

        Result<std::shared_ptr<arrow::ListBuilder>> AffixFuzzer12::new_arrow_array_builder(
            arrow::MemoryPool *memory_pool
        ) {
            if (!memory_pool) {
                return Error(ErrorCode::UnexpectedNullArgument, "Memory pool is null.");
            }

            return Result(std::make_shared<arrow::ListBuilder>(
                memory_pool,
                std::make_shared<arrow::StringBuilder>(memory_pool)
            ));
        }

        Error AffixFuzzer12::fill_arrow_array_builder(
            arrow::ListBuilder *builder, const AffixFuzzer12 *elements, size_t num_elements
        ) {
            if (!builder) {
                return Error(ErrorCode::UnexpectedNullArgument, "Passed array builder is null.");
            }
            if (!elements) {
                return Error(
                    ErrorCode::UnexpectedNullArgument,
                    "Cannot serialize null pointer to arrow array."
                );
            }

            auto value_builder = static_cast<arrow::StringBuilder *>(builder->value_builder());
            ARROW_RETURN_NOT_OK(builder->Reserve(static_cast<int64_t>(num_elements)));
            ARROW_RETURN_NOT_OK(value_builder->Reserve(static_cast<int64_t>(num_elements * 2)));

            for (size_t elem_idx = 0; elem_idx < num_elements; elem_idx += 1) {
                const auto &element = elements[elem_idx];
                ARROW_RETURN_NOT_OK(builder->Append());
                for (size_t item_idx = 0; item_idx < element.many_strings_required.size();
                     item_idx += 1) {
                    ARROW_RETURN_NOT_OK(
                        value_builder->Append(element.many_strings_required[item_idx])
                    );
                }
            }

            return Error::ok();
        }

        Result<rerun::DataCell> AffixFuzzer12::to_data_cell(
            const AffixFuzzer12 *instances, size_t num_instances
        ) {
            // TODO(andreas): Allow configuring the memory pool.
            arrow::MemoryPool *pool = arrow::default_memory_pool();

            auto builder_result = AffixFuzzer12::new_arrow_array_builder(pool);
            RR_RETURN_NOT_OK(builder_result.error);
            auto builder = std::move(builder_result.value);
            if (instances && num_instances > 0) {
                RR_RETURN_NOT_OK(
                    AffixFuzzer12::fill_arrow_array_builder(builder.get(), instances, num_instances)
                );
            }
            std::shared_ptr<arrow::Array> array;
            ARROW_RETURN_NOT_OK(builder->Finish(&array));

            auto schema = arrow::schema(
                {arrow::field(AffixFuzzer12::NAME, AffixFuzzer12::arrow_datatype(), false)}
            );

            rerun::DataCell cell;
            cell.component_name = AffixFuzzer12::NAME;
            const auto ipc_result = rerun::ipc_from_table(*arrow::Table::Make(schema, {array}));
            RR_RETURN_NOT_OK(ipc_result.error);
            cell.buffer = std::move(ipc_result.value);

            return cell;
        }
    } // namespace components
} // namespace rerun
