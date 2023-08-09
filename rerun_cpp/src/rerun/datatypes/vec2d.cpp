// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.
// Based on "crates/re_types/definitions/rerun/datatypes/vec2d.fbs"

#include "vec2d.hpp"

#include <arrow/api.h>

namespace rerun {
    namespace datatypes {
        const std::shared_ptr<arrow::DataType> &Vec2D::to_arrow_datatype() {
            static const auto datatype =
                arrow::fixed_size_list(arrow::field("item", arrow::float32(), false), 2);
            return datatype;
        }

        arrow::Result<std::shared_ptr<arrow::FixedSizeListBuilder>> Vec2D::new_arrow_array_builder(
            arrow::MemoryPool *memory_pool
        ) {
            if (!memory_pool) {
                return arrow::Status::Invalid("Memory pool is null.");
            }

            return arrow::Result(std::make_shared<arrow::FixedSizeListBuilder>(
                memory_pool,
                std::make_shared<arrow::FloatBuilder>(memory_pool),
                2
            ));
        }

        arrow::Status Vec2D::fill_arrow_array_builder(
            arrow::FixedSizeListBuilder *builder, const Vec2D *elements, size_t num_elements
        ) {
            if (!builder) {
                return arrow::Status::Invalid("Passed array builder is null.");
            }
            if (!elements) {
                return arrow::Status::Invalid("Cannot serialize null pointer to arrow array.");
            }

            auto value_builder = static_cast<arrow::FloatBuilder *>(builder->value_builder());

            static_assert(sizeof(elements[0].xy) == sizeof(elements[0]));
            ARROW_RETURN_NOT_OK(
                value_builder->AppendValues(elements[0].xy, num_elements * 2, nullptr)
            );
            ARROW_RETURN_NOT_OK(builder->AppendValues(num_elements));

            return arrow::Status::OK();
        }
    } // namespace datatypes
} // namespace rerun
