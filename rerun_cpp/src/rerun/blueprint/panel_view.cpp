// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/blueprint/panel_view.fbs".

#include "panel_view.hpp"

#include <arrow/builder.h>
#include <arrow/type_fwd.h>

namespace rerun {
    namespace blueprint {
        const std::shared_ptr<arrow::DataType>& PanelView::arrow_datatype() {
            static const auto datatype = arrow::struct_({
                arrow::field("is_expanded", arrow::boolean(), false),
            });
            return datatype;
        }

        Result<std::shared_ptr<arrow::StructBuilder>> PanelView::new_arrow_array_builder(
            arrow::MemoryPool* memory_pool
        ) {
            if (memory_pool == nullptr) {
                return Error(ErrorCode::UnexpectedNullArgument, "Memory pool is null.");
            }

            return Result(std::make_shared<arrow::StructBuilder>(
                arrow_datatype(),
                memory_pool,
                std::vector<std::shared_ptr<arrow::ArrayBuilder>>({
                    std::make_shared<arrow::BooleanBuilder>(memory_pool),
                })
            ));
        }

        Error PanelView::fill_arrow_array_builder(
            arrow::StructBuilder* builder, const PanelView* elements, size_t num_elements
        ) {
            if (builder == nullptr) {
                return Error(ErrorCode::UnexpectedNullArgument, "Passed array builder is null.");
            }
            if (elements == nullptr) {
                return Error(
                    ErrorCode::UnexpectedNullArgument,
                    "Cannot serialize null pointer to arrow array."
                );
            }

            {
                auto field_builder = static_cast<arrow::BooleanBuilder*>(builder->field_builder(0));
                ARROW_RETURN_NOT_OK(field_builder->Reserve(static_cast<int64_t>(num_elements)));
                for (size_t elem_idx = 0; elem_idx < num_elements; elem_idx += 1) {
                    ARROW_RETURN_NOT_OK(field_builder->Append(elements[elem_idx].is_expanded));
                }
            }
            ARROW_RETURN_NOT_OK(builder->AppendValues(static_cast<int64_t>(num_elements), nullptr));

            return Error::ok();
        }
    } // namespace blueprint
} // namespace rerun
