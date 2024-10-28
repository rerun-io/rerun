// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/blueprint/datatypes/filter_is_not_null.fbs".

#include "filter_is_not_null.hpp"

#include "../../datatypes/bool.hpp"
#include "component_column_selector.hpp"

#include <arrow/builder.h>
#include <arrow/type_fwd.h>

namespace rerun::blueprint::datatypes {}

namespace rerun {
    const std::shared_ptr<arrow::DataType>&
        Loggable<blueprint::datatypes::FilterIsNotNull>::arrow_datatype() {
        static const auto datatype = arrow::struct_({
            arrow::field("active", Loggable<rerun::datatypes::Bool>::arrow_datatype(), false),
            arrow::field(
                "column",
                Loggable<rerun::blueprint::datatypes::ComponentColumnSelector>::arrow_datatype(),
                false
            ),
        });
        return datatype;
    }

    Result<std::shared_ptr<arrow::Array>> Loggable<blueprint::datatypes::FilterIsNotNull>::to_arrow(
        const blueprint::datatypes::FilterIsNotNull* instances, size_t num_instances
    ) {
        // TODO(andreas): Allow configuring the memory pool.
        arrow::MemoryPool* pool = arrow::default_memory_pool();
        auto datatype = arrow_datatype();

        ARROW_ASSIGN_OR_RAISE(auto builder, arrow::MakeBuilder(datatype, pool))
        if (instances && num_instances > 0) {
            RR_RETURN_NOT_OK(
                Loggable<blueprint::datatypes::FilterIsNotNull>::fill_arrow_array_builder(
                    static_cast<arrow::StructBuilder*>(builder.get()),
                    instances,
                    num_instances
                )
            );
        }
        std::shared_ptr<arrow::Array> array;
        ARROW_RETURN_NOT_OK(builder->Finish(&array));
        return array;
    }

    rerun::Error Loggable<blueprint::datatypes::FilterIsNotNull>::fill_arrow_array_builder(
        arrow::StructBuilder* builder, const blueprint::datatypes::FilterIsNotNull* elements,
        size_t num_elements
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
            auto field_builder = static_cast<arrow::BooleanBuilder*>(builder->field_builder(0));
            ARROW_RETURN_NOT_OK(field_builder->Reserve(static_cast<int64_t>(num_elements)));
            for (size_t elem_idx = 0; elem_idx < num_elements; elem_idx += 1) {
                RR_RETURN_NOT_OK(Loggable<rerun::datatypes::Bool>::fill_arrow_array_builder(
                    field_builder,
                    &elements[elem_idx].active,
                    1
                ));
            }
        }
        {
            auto field_builder = static_cast<arrow::StructBuilder*>(builder->field_builder(1));
            ARROW_RETURN_NOT_OK(field_builder->Reserve(static_cast<int64_t>(num_elements)));
            for (size_t elem_idx = 0; elem_idx < num_elements; elem_idx += 1) {
                RR_RETURN_NOT_OK(
                    Loggable<rerun::blueprint::datatypes::ComponentColumnSelector>::
                        fill_arrow_array_builder(field_builder, &elements[elem_idx].column, 1)
                );
            }
        }
        ARROW_RETURN_NOT_OK(builder->AppendValues(static_cast<int64_t>(num_elements), nullptr));

        return Error::ok();
    }
} // namespace rerun
