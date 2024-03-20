// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/testing/datatypes/fuzzy.fbs".

#include "affix_fuzzer5.hpp"

#include "affix_fuzzer4.hpp"

#include <arrow/builder.h>
#include <arrow/type_fwd.h>

namespace rerun::datatypes {}

namespace rerun {
    const std::shared_ptr<arrow::DataType>& Loggable<datatypes::AffixFuzzer5>::arrow_datatype() {
        static const auto datatype = arrow::struct_({
            arrow::field(
                "single_optional_union",
                Loggable<rerun::datatypes::AffixFuzzer4>::arrow_datatype(),
                true
            ),
        });
        return datatype;
    }

    Result<std::shared_ptr<arrow::Array>> Loggable<datatypes::AffixFuzzer5>::to_arrow(
        const datatypes::AffixFuzzer5* instances, size_t num_instances
    ) {
        // TODO(andreas): Allow configuring the memory pool.
        arrow::MemoryPool* pool = arrow::default_memory_pool();
        auto datatype = arrow_datatype();

        ARROW_ASSIGN_OR_RAISE(auto builder, arrow::MakeBuilder(datatype, pool))
        if (instances && num_instances > 0) {
            RR_RETURN_NOT_OK(Loggable<datatypes::AffixFuzzer5>::fill_arrow_array_builder(
                static_cast<arrow::StructBuilder*>(builder.get()),
                instances,
                num_instances
            ));
        }
        std::shared_ptr<arrow::Array> array;
        ARROW_RETURN_NOT_OK(builder->Finish(&array));
        return array;
    }

    rerun::Error Loggable<datatypes::AffixFuzzer5>::fill_arrow_array_builder(
        arrow::StructBuilder* builder, const datatypes::AffixFuzzer5* elements, size_t num_elements
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

        {
            auto field_builder = static_cast<arrow::DenseUnionBuilder*>(builder->field_builder(0));
            ARROW_RETURN_NOT_OK(field_builder->Reserve(static_cast<int64_t>(num_elements)));
            for (size_t elem_idx = 0; elem_idx < num_elements; elem_idx += 1) {
                const auto& element = elements[elem_idx];
                if (element.single_optional_union.has_value()) {
                    RR_RETURN_NOT_OK(
                        Loggable<rerun::datatypes::AffixFuzzer4>::fill_arrow_array_builder(
                            field_builder,
                            &element.single_optional_union.value(),
                            1
                        )
                    );
                } else {
                    ARROW_RETURN_NOT_OK(field_builder->AppendNull());
                }
            }
        }
        ARROW_RETURN_NOT_OK(builder->AppendValues(static_cast<int64_t>(num_elements), nullptr));

        return Error::ok();
    }
} // namespace rerun
