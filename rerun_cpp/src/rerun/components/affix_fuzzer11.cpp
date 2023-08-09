// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.
// Based on "crates/re_types/definitions/rerun/testing/components/fuzzy.fbs"

#include "affix_fuzzer11.hpp"

#include "../arrow.hpp"

#include <arrow/api.h>

namespace rerun {
    namespace components {
        const char *AffixFuzzer11::NAME = "rerun.testing.components.AffixFuzzer11";

        const std::shared_ptr<arrow::DataType> &AffixFuzzer11::to_arrow_datatype() {
            static const auto datatype = arrow::list(arrow::field("item", arrow::float32(), false));
            return datatype;
        }

        arrow::Result<std::shared_ptr<arrow::ListBuilder>> AffixFuzzer11::new_arrow_array_builder(
            arrow::MemoryPool *memory_pool
        ) {
            if (!memory_pool) {
                return arrow::Status::Invalid("Memory pool is null.");
            }

            return arrow::Result(std::make_shared<arrow::ListBuilder>(
                memory_pool,
                std::make_shared<arrow::FloatBuilder>(memory_pool)
            ));
        }

        arrow::Status AffixFuzzer11::fill_arrow_array_builder(
            arrow::ListBuilder *builder, const AffixFuzzer11 *elements, size_t num_elements
        ) {
            if (!builder) {
                return arrow::Status::Invalid("Passed array builder is null.");
            }
            if (!elements) {
                return arrow::Status::Invalid("Cannot serialize null pointer to arrow array.");
            }

            auto value_builder = static_cast<arrow::FloatBuilder *>(builder->value_builder());
            ARROW_RETURN_NOT_OK(builder->Reserve(num_elements));
            ARROW_RETURN_NOT_OK(value_builder->Reserve(num_elements * 1));

            for (auto elem_idx = 0; elem_idx < num_elements; elem_idx += 1) {
                const auto &element = elements[elem_idx];
                if (element.many_floats_optional.has_value()) {
                    ARROW_RETURN_NOT_OK(value_builder->AppendValues(
                        element.many_floats_optional.value().data(),
                        element.many_floats_optional.value().size(),
                        nullptr
                    ));
                    ARROW_RETURN_NOT_OK(builder->Append());
                } else {
                    ARROW_RETURN_NOT_OK(builder->AppendNull());
                }
            }

            return arrow::Status::OK();
        }

        arrow::Result<rerun::DataCell> AffixFuzzer11::to_data_cell(
            const AffixFuzzer11 *instances, size_t num_instances
        ) {
            // TODO(andreas): Allow configuring the memory pool.
            arrow::MemoryPool *pool = arrow::default_memory_pool();

            ARROW_ASSIGN_OR_RAISE(auto builder, AffixFuzzer11::new_arrow_array_builder(pool));
            if (instances && num_instances > 0) {
                ARROW_RETURN_NOT_OK(
                    AffixFuzzer11::fill_arrow_array_builder(builder.get(), instances, num_instances)
                );
            }
            std::shared_ptr<arrow::Array> array;
            ARROW_RETURN_NOT_OK(builder->Finish(&array));

            auto schema = arrow::schema(
                {arrow::field(AffixFuzzer11::NAME, AffixFuzzer11::to_arrow_datatype(), false)}
            );

            rerun::DataCell cell;
            cell.component_name = AffixFuzzer11::NAME;
            ARROW_ASSIGN_OR_RAISE(
                cell.buffer,
                rerun::ipc_from_table(*arrow::Table::Make(schema, {array}))
            );

            return cell;
        }
    } // namespace components
} // namespace rerun
