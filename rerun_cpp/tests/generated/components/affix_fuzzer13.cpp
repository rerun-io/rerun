// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/testing/components/fuzzy.fbs".

#include "affix_fuzzer13.hpp"

#include <arrow/builder.h>
#include <arrow/type_fwd.h>

namespace rerun::components {}

namespace rerun {
    const std::shared_ptr<arrow::DataType>& Loggable<components::AffixFuzzer13>::arrow_datatype() {
        static const auto datatype = arrow::list(arrow::field("item", arrow::utf8(), false));
        return datatype;
    }

    Result<std::shared_ptr<arrow::Array>> Loggable<components::AffixFuzzer13>::to_arrow(
        const components::AffixFuzzer13* instances, size_t num_instances
    ) {
        // TODO(andreas): Allow configuring the memory pool.
        arrow::MemoryPool* pool = arrow::default_memory_pool();
        auto datatype = arrow_datatype();

        ARROW_ASSIGN_OR_RAISE(auto builder, arrow::MakeBuilder(datatype, pool))
        if (instances && num_instances > 0) {
            RR_RETURN_NOT_OK(Loggable<components::AffixFuzzer13>::fill_arrow_array_builder(
                static_cast<arrow::ListBuilder*>(builder.get()),
                instances,
                num_instances
            ));
        }
        std::shared_ptr<arrow::Array> array;
        ARROW_RETURN_NOT_OK(builder->Finish(&array));
        return array;
    }

    rerun::Error Loggable<components::AffixFuzzer13>::fill_arrow_array_builder(
        arrow::ListBuilder* builder, const components::AffixFuzzer13* elements, size_t num_elements
    ) {
        if (builder == nullptr) {
            return rerun::Error(ErrorCode::UnexpectedNullArgument, "Passed array builder is null.");
        }
        if (elements == nullptr) {
            return rerun::Error(
                ErrorCode::UnexpectedNullArgument,
                "Cannot serialize null pointer to arrow array."
            );
        }

        auto value_builder = static_cast<arrow::StringBuilder*>(builder->value_builder());
        ARROW_RETURN_NOT_OK(builder->Reserve(static_cast<int64_t>(num_elements)));
        ARROW_RETURN_NOT_OK(value_builder->Reserve(static_cast<int64_t>(num_elements * 1)));

        for (size_t elem_idx = 0; elem_idx < num_elements; elem_idx += 1) {
            const auto& element = elements[elem_idx];
            if (element.many_strings_optional.has_value()) {
                ARROW_RETURN_NOT_OK(builder->Append());
                for (size_t item_idx = 0; item_idx < element.many_strings_optional.value().size();
                     item_idx += 1) {
                    ARROW_RETURN_NOT_OK(value_builder->Append(element.many_strings_optional.value(
                    )[item_idx]));
                }
            } else {
                ARROW_RETURN_NOT_OK(builder->AppendNull());
            }
        }

        return Error::ok();
    }
} // namespace rerun
