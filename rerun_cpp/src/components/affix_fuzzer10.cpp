// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.
// Based on "crates/re_types/definitions/rerun/testing/components/fuzzy.fbs"

#include "affix_fuzzer10.hpp"

#include "../rerun.hpp"

#include <arrow/api.h>

namespace rr {
    namespace components {
        const char* AffixFuzzer10::NAME = "rerun.testing.components.AffixFuzzer10";

        const std::shared_ptr<arrow::DataType>& AffixFuzzer10::to_arrow_datatype() {
            static const auto datatype = arrow::utf8();
            return datatype;
        }

        arrow::Result<std::shared_ptr<arrow::StringBuilder>> AffixFuzzer10::new_arrow_array_builder(
            arrow::MemoryPool* memory_pool
        ) {
            if (!memory_pool) {
                return arrow::Status::Invalid("Memory pool is null.");
            }

            return arrow::Result(std::make_shared<arrow::StringBuilder>(memory_pool));
        }

        arrow::Status AffixFuzzer10::fill_arrow_array_builder(
            arrow::StringBuilder* builder, const AffixFuzzer10* elements, size_t num_elements
        ) {
            if (!builder) {
                return arrow::Status::Invalid("Passed array builder is null.");
            }
            if (!elements) {
                return arrow::Status::Invalid("Cannot serialize null pointer to arrow array.");
            }

            ARROW_RETURN_NOT_OK(builder->Reserve(num_elements));
            for (auto elem_idx = 0; elem_idx < num_elements; elem_idx += 1) {
                const auto& element = elements[elem_idx];
                if (element.single_string_optional.has_value()) {
                    ARROW_RETURN_NOT_OK(builder->Append(element.single_string_optional.value()));
                } else {
                    ARROW_RETURN_NOT_OK(builder->AppendNull());
                }
            }

            return arrow::Status::OK();
        }

        arrow::Result<rr::DataCell> AffixFuzzer10::to_data_cell(
            const AffixFuzzer10* components, size_t num_components
        ) {
            // TODO(andreas): Allow configuring the memory pool.
            arrow::MemoryPool* pool = arrow::default_memory_pool();

            ARROW_ASSIGN_OR_RAISE(auto builder, AffixFuzzer10::new_arrow_array_builder(pool));
            if (components && num_components > 0) {
                ARROW_RETURN_NOT_OK(AffixFuzzer10::fill_arrow_array_builder(
                    builder.get(),
                    components,
                    num_components
                ));
            }
            std::shared_ptr<arrow::Array> array;
            ARROW_RETURN_NOT_OK(builder->Finish(&array));

            auto schema = arrow::schema(
                {arrow::field(AffixFuzzer10::NAME, AffixFuzzer10::to_arrow_datatype(), false)}
            );

            rr::DataCell cell;
            cell.component_name = AffixFuzzer10::NAME;
            ARROW_ASSIGN_OR_RAISE(
                cell.buffer,
                rr::ipc_from_table(*arrow::Table::Make(schema, {array}))
            );

            return cell;
        }
    } // namespace components
} // namespace rr
