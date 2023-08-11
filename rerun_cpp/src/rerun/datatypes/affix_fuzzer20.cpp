// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.
// Based on "crates/re_types/definitions/rerun/testing/datatypes/fuzzy.fbs"

#include "affix_fuzzer20.hpp"

#include "../components/primitive_component.hpp"
#include "../components/string_component.hpp"

#include <arrow/api.h>

namespace rerun {
    namespace datatypes {
        const std::shared_ptr<arrow::DataType> &AffixFuzzer20::to_arrow_datatype() {
            static const auto datatype = arrow::struct_({
                arrow::field(
                    "p",
                    rerun::components::PrimitiveComponent::to_arrow_datatype(),
                    false
                ),
                arrow::field("s", rerun::components::StringComponent::to_arrow_datatype(), false),
            });
            return datatype;
        }

        arrow::Result<std::shared_ptr<arrow::StructBuilder>> AffixFuzzer20::new_arrow_array_builder(
            arrow::MemoryPool *memory_pool
        ) {
            if (!memory_pool) {
                return arrow::Status::Invalid("Memory pool is null.");
            }

            return arrow::Result(std::make_shared<arrow::StructBuilder>(
                to_arrow_datatype(),
                memory_pool,
                std::vector<std::shared_ptr<arrow::ArrayBuilder>>({
                    rerun::components::PrimitiveComponent::new_arrow_array_builder(memory_pool)
                        .ValueOrDie(),
                    rerun::components::StringComponent::new_arrow_array_builder(memory_pool)
                        .ValueOrDie(),
                })
            ));
        }

        arrow::Status AffixFuzzer20::fill_arrow_array_builder(
            arrow::StructBuilder *builder, const AffixFuzzer20 *elements, size_t num_elements
        ) {
            if (!builder) {
                return arrow::Status::Invalid("Passed array builder is null.");
            }
            if (!elements) {
                return arrow::Status::Invalid("Cannot serialize null pointer to arrow array.");
            }

            {
                auto field_builder = static_cast<arrow::UInt32Builder *>(builder->field_builder(0));
                ARROW_RETURN_NOT_OK(field_builder->Reserve(num_elements));
                for (auto elem_idx = 0; elem_idx < num_elements; elem_idx += 1) {
                    ARROW_RETURN_NOT_OK(
                        rerun::components::PrimitiveComponent::fill_arrow_array_builder(
                            field_builder,
                            &elements[elem_idx].p,
                            1
                        )
                    );
                }
            }
            {
                auto field_builder = static_cast<arrow::StringBuilder *>(builder->field_builder(1));
                ARROW_RETURN_NOT_OK(field_builder->Reserve(num_elements));
                for (auto elem_idx = 0; elem_idx < num_elements; elem_idx += 1) {
                    ARROW_RETURN_NOT_OK(
                        rerun::components::StringComponent::fill_arrow_array_builder(
                            field_builder,
                            &elements[elem_idx].s,
                            1
                        )
                    );
                }
            }
            ARROW_RETURN_NOT_OK(builder->AppendValues(num_elements, nullptr));

            return arrow::Status::OK();
        }
    } // namespace datatypes
} // namespace rerun
