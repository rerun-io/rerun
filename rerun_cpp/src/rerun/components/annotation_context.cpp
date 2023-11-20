// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/components/annotation_context.fbs".

#include "annotation_context.hpp"

#include "../datatypes/class_description_map_elem.hpp"

#include <arrow/builder.h>
#include <arrow/type_fwd.h>

namespace rerun::components {
    const char AnnotationContext::NAME[] = "rerun.components.AnnotationContext";

    const std::shared_ptr<arrow::DataType>& AnnotationContext::arrow_datatype() {
        static const auto datatype = arrow::list(
            arrow::field("item", rerun::datatypes::ClassDescriptionMapElem::arrow_datatype(), false)
        );
        return datatype;
    }

    rerun::Error AnnotationContext::fill_arrow_array_builder(
        arrow::ListBuilder* builder, const AnnotationContext* elements, size_t num_elements
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

        auto value_builder = static_cast<arrow::StructBuilder*>(builder->value_builder());
        ARROW_RETURN_NOT_OK(builder->Reserve(static_cast<int64_t>(num_elements)));
        ARROW_RETURN_NOT_OK(value_builder->Reserve(static_cast<int64_t>(num_elements * 2)));

        for (size_t elem_idx = 0; elem_idx < num_elements; elem_idx += 1) {
            const auto& element = elements[elem_idx];
            ARROW_RETURN_NOT_OK(builder->Append());
            if (element.class_map.data()) {
                RR_RETURN_NOT_OK(
                    rerun::datatypes::ClassDescriptionMapElem::fill_arrow_array_builder(
                        value_builder,
                        element.class_map.data(),
                        element.class_map.size()
                    )
                );
            }
        }

        return Error::ok();
    }

    Result<rerun::DataCell> AnnotationContext::to_data_cell(
        const AnnotationContext* instances, size_t num_instances
    ) {
        // TODO(andreas): Allow configuring the memory pool.
        arrow::MemoryPool* pool = arrow::default_memory_pool();

        ARROW_ASSIGN_OR_RAISE(auto builder, arrow::MakeBuilder(arrow_datatype(), pool));
        if (instances && num_instances > 0) {
            RR_RETURN_NOT_OK(AnnotationContext::fill_arrow_array_builder(
                static_cast<arrow::ListBuilder*>(builder.get()),
                instances,
                num_instances
            ));
        }
        std::shared_ptr<arrow::Array> array;
        ARROW_RETURN_NOT_OK(builder->Finish(&array));

        return rerun::DataCell::create(
            AnnotationContext::NAME,
            AnnotationContext::arrow_datatype(),
            std::move(array)
        );
    }
} // namespace rerun::components
