// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/testing/components/fuzzy.fbs".

#include "affix_fuzzer11.hpp"

#include <arrow/builder.h>
#include <arrow/type_fwd.h>

namespace rerun::components {
    const char AffixFuzzer11::NAME[] = "rerun.testing.components.AffixFuzzer11";

    const std::shared_ptr<arrow::DataType>& AffixFuzzer11::arrow_datatype() {
        static const auto datatype = arrow::list(arrow::field("item", arrow::float32(), false));
        return datatype;
    }

    rerun::Error AffixFuzzer11::fill_arrow_array_builder(
        arrow::ListBuilder* builder, const AffixFuzzer11* elements, size_t num_elements
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

        auto value_builder = static_cast<arrow::FloatBuilder*>(builder->value_builder());
        ARROW_RETURN_NOT_OK(builder->Reserve(static_cast<int64_t>(num_elements)));
        ARROW_RETURN_NOT_OK(value_builder->Reserve(static_cast<int64_t>(num_elements * 1)));

        for (size_t elem_idx = 0; elem_idx < num_elements; elem_idx += 1) {
            const auto& element = elements[elem_idx];
            if (element.many_floats_optional.has_value()) {
                ARROW_RETURN_NOT_OK(builder->Append());
                ARROW_RETURN_NOT_OK(value_builder->AppendValues(
                    element.many_floats_optional.value().data(),
                    static_cast<int64_t>(element.many_floats_optional.value().size()),
                    nullptr
                ));
            } else {
                ARROW_RETURN_NOT_OK(builder->AppendNull());
            }
        }

        return Error::ok();
    }

    Result<rerun::DataCell> AffixFuzzer11::to_data_cell(
        const AffixFuzzer11* instances, size_t num_instances
    ) {
        // TODO(andreas): Allow configuring the memory pool.
        arrow::MemoryPool* pool = arrow::default_memory_pool();
        auto datatype = arrow_datatype();

        ARROW_ASSIGN_OR_RAISE(auto builder, arrow::MakeBuilder(datatype, pool))
        if (instances && num_instances > 0) {
            RR_RETURN_NOT_OK(AffixFuzzer11::fill_arrow_array_builder(
                static_cast<arrow::ListBuilder*>(builder.get()),
                instances,
                num_instances
            ));
        }
        std::shared_ptr<arrow::Array> array;
        ARROW_RETURN_NOT_OK(builder->Finish(&array));

        static const Result<ComponentTypeHandle> component_type =
            ComponentType(NAME, datatype).register_component();
        RR_RETURN_NOT_OK(component_type.error);

        DataCell cell;
        cell.num_instances = num_instances;
        cell.array = std::move(array);
        cell.component_type = component_type.value;
        return cell;
    }
} // namespace rerun::components
