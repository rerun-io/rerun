// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.
// Based on "crates/re_types/definitions/rerun/testing/components/fuzzy.fbs"

#include "affix_fuzzer18.hpp"

#include "../datatypes/affix_fuzzer4.hpp"

#include <arrow/builder.h>
#include <arrow/table.h>
#include <arrow/type_fwd.h>
#include <rerun/arrow.hpp>

namespace rerun {
    namespace components {
        const char *AffixFuzzer18::NAME = "rerun.testing.components.AffixFuzzer18";

        const std::shared_ptr<arrow::DataType> &AffixFuzzer18::arrow_datatype() {
            static const auto datatype = arrow::list(
                arrow::field("item", rerun::datatypes::AffixFuzzer4::arrow_datatype(), false)
            );
            return datatype;
        }

        Result<std::shared_ptr<arrow::ListBuilder>> AffixFuzzer18::new_arrow_array_builder(
            arrow::MemoryPool *memory_pool
        ) {
            if (!memory_pool) {
                return Error(ErrorCode::UnexpectedNullArgument, "Memory pool is null.");
            }

            return Result(std::make_shared<arrow::ListBuilder>(
                memory_pool,
                rerun::datatypes::AffixFuzzer4::new_arrow_array_builder(memory_pool).value
            ));
        }

        Error AffixFuzzer18::fill_arrow_array_builder(
            arrow::ListBuilder *builder, const AffixFuzzer18 *elements, size_t num_elements
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

            auto value_builder = static_cast<arrow::DenseUnionBuilder *>(builder->value_builder());
            ARROW_RETURN_NOT_OK(builder->Reserve(static_cast<int64_t>(num_elements)));
            ARROW_RETURN_NOT_OK(value_builder->Reserve(static_cast<int64_t>(num_elements * 1)));

            for (size_t elem_idx = 0; elem_idx < num_elements; elem_idx += 1) {
                const auto &element = elements[elem_idx];
                if (element.many_optional_unions.has_value()) {
                    ARROW_RETURN_NOT_OK(builder->Append());
                    if (element.many_optional_unions.value().data()) {
                        RR_RETURN_NOT_OK(rerun::datatypes::AffixFuzzer4::fill_arrow_array_builder(
                            value_builder,
                            element.many_optional_unions.value().data(),
                            element.many_optional_unions.value().size()
                        ));
                    }
                } else {
                    ARROW_RETURN_NOT_OK(builder->AppendNull());
                }
            }

            return Error::ok();
        }

        Result<rerun::DataCell> AffixFuzzer18::to_data_cell(
            const AffixFuzzer18 *instances, size_t num_instances
        ) {
            // TODO(andreas): Allow configuring the memory pool.
            arrow::MemoryPool *pool = arrow::default_memory_pool();

            auto builder_result = AffixFuzzer18::new_arrow_array_builder(pool);
            RR_RETURN_NOT_OK(builder_result.error);
            auto builder = std::move(builder_result.value);
            if (instances && num_instances > 0) {
                RR_RETURN_NOT_OK(
                    AffixFuzzer18::fill_arrow_array_builder(builder.get(), instances, num_instances)
                );
            }
            std::shared_ptr<arrow::Array> array;
            ARROW_RETURN_NOT_OK(builder->Finish(&array));

            auto schema = arrow::schema(
                {arrow::field(AffixFuzzer18::NAME, AffixFuzzer18::arrow_datatype(), false)}
            );

            rerun::DataCell cell;
            cell.component_name = AffixFuzzer18::NAME;
            const auto ipc_result = rerun::ipc_from_table(*arrow::Table::Make(schema, {array}));
            RR_RETURN_NOT_OK(ipc_result.error);
            cell.buffer = std::move(ipc_result.value);

            return cell;
        }
    } // namespace components
} // namespace rerun
