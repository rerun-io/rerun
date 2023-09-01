// DO NOT EDIT!: This file was autogenerated by re_types_builder in
// crates/re_types_builder/src/codegen/cpp/mod.rs:54 Based on
// "crates/re_types/definitions/rerun/datatypes/vec2d.fbs"

#include "vec2d.hpp"

#include <arrow/builder.h>
#include <arrow/type_fwd.h>

namespace rerun {
    namespace datatypes {
        const std::shared_ptr<arrow::DataType> &Vec2D::arrow_datatype() {
            static const auto datatype =
                arrow::fixed_size_list(arrow::field("item", arrow::float32(), false), 2);
            return datatype;
        }

        Result<std::shared_ptr<arrow::FixedSizeListBuilder>> Vec2D::new_arrow_array_builder(
            arrow::MemoryPool *memory_pool
        ) {
            if (!memory_pool) {
                return Error(ErrorCode::UnexpectedNullArgument, "Memory pool is null.");
            }

            return Result(std::make_shared<arrow::FixedSizeListBuilder>(
                memory_pool,
                std::make_shared<arrow::FloatBuilder>(memory_pool),
                2
            ));
        }

        Error Vec2D::fill_arrow_array_builder(
            arrow::FixedSizeListBuilder *builder, const Vec2D *elements, size_t num_elements
        ) {
            if (!builder) {
                return Error(ErrorCode::UnexpectedNullArgument, "Passed array builder is null.");
            }
            if (!elements) {
                return Error(
                    ErrorCode::UnexpectedNullArgument,
                    "Cannot serialize null pointer to arrow array."
                );
            }

            auto value_builder = static_cast<arrow::FloatBuilder *>(builder->value_builder());

            ARROW_RETURN_NOT_OK(builder->AppendValues(static_cast<int64_t>(num_elements)));
            static_assert(sizeof(elements[0].xy) == sizeof(elements[0]));
            ARROW_RETURN_NOT_OK(value_builder->AppendValues(
                elements[0].xy,
                static_cast<int64_t>(num_elements * 2),
                nullptr
            ));

            return Error::ok();
        }
    } // namespace datatypes
} // namespace rerun
