// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/blueprint/datatypes/selected_columns.fbs".

#include "selected_columns.hpp"

#include "../../datatypes/utf8.hpp"
#include "component_column_selector.hpp"

#include <arrow/builder.h>
#include <arrow/type_fwd.h>

namespace rerun::blueprint::datatypes {}

namespace rerun {
    const std::shared_ptr<arrow::DataType>&
        Loggable<blueprint::datatypes::SelectedColumns>::arrow_datatype() {
        static const auto datatype = arrow::struct_({
            arrow::field(
                "time_columns",
                arrow::list(
                    arrow::field("item", Loggable<rerun::datatypes::Utf8>::arrow_datatype(), false)
                ),
                false
            ),
            arrow::field(
                "component_columns",
                arrow::list(arrow::field(
                    "item",
                    Loggable<rerun::blueprint::datatypes::ComponentColumnSelector>::arrow_datatype(
                    ),
                    false
                )),
                false
            ),
        });
        return datatype;
    }

    Result<std::shared_ptr<arrow::Array>> Loggable<blueprint::datatypes::SelectedColumns>::to_arrow(
        const blueprint::datatypes::SelectedColumns* instances, size_t num_instances
    ) {
        // TODO(andreas): Allow configuring the memory pool.
        arrow::MemoryPool* pool = arrow::default_memory_pool();
        auto datatype = arrow_datatype();

        ARROW_ASSIGN_OR_RAISE(auto builder, arrow::MakeBuilder(datatype, pool))
        if (instances && num_instances > 0) {
            RR_RETURN_NOT_OK(
                Loggable<blueprint::datatypes::SelectedColumns>::fill_arrow_array_builder(
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

    rerun::Error Loggable<blueprint::datatypes::SelectedColumns>::fill_arrow_array_builder(
        arrow::StructBuilder* builder, const blueprint::datatypes::SelectedColumns* elements,
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
            auto field_builder = static_cast<arrow::ListBuilder*>(builder->field_builder(0));
            auto value_builder = static_cast<arrow::StringBuilder*>(field_builder->value_builder());
            ARROW_RETURN_NOT_OK(field_builder->Reserve(static_cast<int64_t>(num_elements)));
            ARROW_RETURN_NOT_OK(value_builder->Reserve(static_cast<int64_t>(num_elements * 2)));

            for (size_t elem_idx = 0; elem_idx < num_elements; elem_idx += 1) {
                const auto& element = elements[elem_idx];
                ARROW_RETURN_NOT_OK(field_builder->Append());
                if (element.time_columns.data()) {
                    RR_RETURN_NOT_OK(Loggable<rerun::datatypes::Utf8>::fill_arrow_array_builder(
                        value_builder,
                        element.time_columns.data(),
                        element.time_columns.size()
                    ));
                }
            }
        }
        {
            auto field_builder = static_cast<arrow::ListBuilder*>(builder->field_builder(1));
            auto value_builder = static_cast<arrow::StructBuilder*>(field_builder->value_builder());
            ARROW_RETURN_NOT_OK(field_builder->Reserve(static_cast<int64_t>(num_elements)));
            ARROW_RETURN_NOT_OK(value_builder->Reserve(static_cast<int64_t>(num_elements * 2)));

            for (size_t elem_idx = 0; elem_idx < num_elements; elem_idx += 1) {
                const auto& element = elements[elem_idx];
                ARROW_RETURN_NOT_OK(field_builder->Append());
                if (element.component_columns.data()) {
                    RR_RETURN_NOT_OK(
                        Loggable<rerun::blueprint::datatypes::ComponentColumnSelector>::
                            fill_arrow_array_builder(
                                value_builder,
                                element.component_columns.data(),
                                element.component_columns.size()
                            )
                    );
                }
            }
        }
        ARROW_RETURN_NOT_OK(builder->AppendValues(static_cast<int64_t>(num_elements), nullptr));

        return Error::ok();
    }
} // namespace rerun
