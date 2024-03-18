// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/testing/components/fuzzy.fbs".

#include "affix_fuzzer8.hpp"

#include <arrow/builder.h>
#include <arrow/type_fwd.h>

namespace rerun::components {}

namespace rerun {
    const std::shared_ptr<arrow::DataType>& Loggable<components::AffixFuzzer8>::arrow_datatype() {
        static const auto datatype = arrow::float32();
        return datatype;
    }

    Result<std::shared_ptr<arrow::Array>> Loggable<components::AffixFuzzer8>::to_arrow(
        const components::AffixFuzzer8* instances, size_t num_instances
    ) {
        // TODO(andreas): Allow configuring the memory pool.
        arrow::MemoryPool* pool = arrow::default_memory_pool();
        auto datatype = arrow_datatype();

        ARROW_ASSIGN_OR_RAISE(auto builder, arrow::MakeBuilder(datatype, pool))
        if (instances && num_instances > 0) {
            RR_RETURN_NOT_OK(Loggable<components::AffixFuzzer8>::fill_arrow_array_builder(
                static_cast<arrow::FloatBuilder*>(builder.get()),
                instances,
                num_instances
            ));
        }
        std::shared_ptr<arrow::Array> array;
        ARROW_RETURN_NOT_OK(builder->Finish(&array));
        return array;
    }

    rerun::Error Loggable<components::AffixFuzzer8>::fill_arrow_array_builder(
        arrow::FloatBuilder* builder, const components::AffixFuzzer8* elements, size_t num_elements
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

        ARROW_RETURN_NOT_OK(builder->Reserve(static_cast<int64_t>(num_elements)));
        for (size_t elem_idx = 0; elem_idx < num_elements; elem_idx += 1) {
            const auto& element = elements[elem_idx];
            if (element.single_float_optional.has_value()) {
                ARROW_RETURN_NOT_OK(builder->Append(element.single_float_optional.value()));
            } else {
                ARROW_RETURN_NOT_OK(builder->AppendNull());
            }
        }

        return Error::ok();
    }
} // namespace rerun
