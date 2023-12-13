// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/testing/datatypes/fuzzy.fbs".

#include "affix_fuzzer22.hpp"

#include <arrow/builder.h>
#include <arrow/type_fwd.h>

namespace rerun::datatypes {}

namespace rerun {
    const std::shared_ptr<arrow::DataType>& Loggable<datatypes::AffixFuzzer22>::arrow_datatype() {
        static const auto datatype = arrow::struct_({
            arrow::field(
                "fixed_sized_native",
                arrow::fixed_size_list(arrow::field("item", arrow::uint8(), false), 4),
                false
            ),
        });
        return datatype;
    }

    rerun::Error Loggable<datatypes::AffixFuzzer22>::fill_arrow_array_builder(
        arrow::StructBuilder* builder, const datatypes::AffixFuzzer22* elements, size_t num_elements
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
            auto field_builder =
                static_cast<arrow::FixedSizeListBuilder*>(builder->field_builder(0));
            auto value_builder = static_cast<arrow::UInt8Builder*>(field_builder->value_builder());

            ARROW_RETURN_NOT_OK(field_builder->AppendValues(static_cast<int64_t>(num_elements)));
            static_assert(sizeof(elements[0].fixed_sized_native) == sizeof(elements[0]));
            ARROW_RETURN_NOT_OK(value_builder->AppendValues(
                elements[0].fixed_sized_native.data(),
                static_cast<int64_t>(num_elements * 4),
                nullptr
            ));
        }
        ARROW_RETURN_NOT_OK(builder->AppendValues(static_cast<int64_t>(num_elements), nullptr));

        return Error::ok();
    }

    Result<std::shared_ptr<arrow::Array>> Loggable<datatypes::AffixFuzzer22>::to_arrow(
        const datatypes::AffixFuzzer22* instances, size_t num_instances
    ) {
        // TODO(andreas): Allow configuring the memory pool.
        arrow::MemoryPool* pool = arrow::default_memory_pool();
        auto datatype = arrow_datatype();

        ARROW_ASSIGN_OR_RAISE(auto builder, arrow::MakeBuilder(datatype, pool))
        if (instances && num_instances > 0) {
            RR_RETURN_NOT_OK(Loggable<datatypes::AffixFuzzer22>::fill_arrow_array_builder(
                static_cast<arrow::StructBuilder*>(builder.get()),
                instances,
                num_instances
            ));
        }
        std::shared_ptr<arrow::Array> array;
        ARROW_RETURN_NOT_OK(builder->Finish(&array));
        return array;
    }
} // namespace rerun
