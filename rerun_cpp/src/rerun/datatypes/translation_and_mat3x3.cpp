// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/datatypes/translation_and_mat3x3.fbs".

#include "translation_and_mat3x3.hpp"

#include "mat3x3.hpp"
#include "vec3d.hpp"

#include <arrow/builder.h>
#include <arrow/type_fwd.h>

namespace rerun::datatypes {
    const std::shared_ptr<arrow::DataType>& TranslationAndMat3x3::arrow_datatype() {
        static const auto datatype = arrow::struct_({
            arrow::field("translation", rerun::datatypes::Vec3D::arrow_datatype(), true),
            arrow::field("mat3x3", rerun::datatypes::Mat3x3::arrow_datatype(), true),
            arrow::field("from_parent", arrow::boolean(), false),
        });
        return datatype;
    }

    Result<std::shared_ptr<arrow::StructBuilder>> TranslationAndMat3x3::new_arrow_array_builder(
        arrow::MemoryPool* memory_pool
    ) {
        if (memory_pool == nullptr) {
            return rerun::Error(ErrorCode::UnexpectedNullArgument, "Memory pool is null.");
        }

        return Result(std::make_shared<arrow::StructBuilder>(
            arrow_datatype(),
            memory_pool,
            std::vector<std::shared_ptr<arrow::ArrayBuilder>>({
                rerun::datatypes::Vec3D::new_arrow_array_builder(memory_pool).value,
                rerun::datatypes::Mat3x3::new_arrow_array_builder(memory_pool).value,
                std::make_shared<arrow::BooleanBuilder>(memory_pool),
            })
        ));
    }

    rerun::Error TranslationAndMat3x3::fill_arrow_array_builder(
        arrow::StructBuilder* builder, const TranslationAndMat3x3* elements, size_t num_elements
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
            ARROW_RETURN_NOT_OK(field_builder->Reserve(static_cast<int64_t>(num_elements)));
            for (size_t elem_idx = 0; elem_idx < num_elements; elem_idx += 1) {
                const auto& element = elements[elem_idx];
                if (element.translation.has_value()) {
                    RR_RETURN_NOT_OK(rerun::datatypes::Vec3D::fill_arrow_array_builder(
                        field_builder,
                        &element.translation.value(),
                        1
                    ));
                } else {
                    ARROW_RETURN_NOT_OK(field_builder->AppendNull());
                }
            }
        }
        {
            auto field_builder =
                static_cast<arrow::FixedSizeListBuilder*>(builder->field_builder(1));
            ARROW_RETURN_NOT_OK(field_builder->Reserve(static_cast<int64_t>(num_elements)));
            for (size_t elem_idx = 0; elem_idx < num_elements; elem_idx += 1) {
                const auto& element = elements[elem_idx];
                if (element.mat3x3.has_value()) {
                    RR_RETURN_NOT_OK(rerun::datatypes::Mat3x3::fill_arrow_array_builder(
                        field_builder,
                        &element.mat3x3.value(),
                        1
                    ));
                } else {
                    ARROW_RETURN_NOT_OK(field_builder->AppendNull());
                }
            }
        }
        {
            auto field_builder = static_cast<arrow::BooleanBuilder*>(builder->field_builder(2));
            ARROW_RETURN_NOT_OK(field_builder->Reserve(static_cast<int64_t>(num_elements)));
            for (size_t elem_idx = 0; elem_idx < num_elements; elem_idx += 1) {
                ARROW_RETURN_NOT_OK(field_builder->Append(elements[elem_idx].from_parent));
            }
        }
        ARROW_RETURN_NOT_OK(builder->AppendValues(static_cast<int64_t>(num_elements), nullptr));

        return Error::ok();
    }
} // namespace rerun::datatypes
