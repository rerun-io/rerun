// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.
// Based on "crates/re_types/definitions/rerun/datatypes/translation_rotation_scale3d.fbs"

#include "translation_rotation_scale3d.hpp"

#include "rotation3d.hpp"
#include "scale3d.hpp"
#include "vec3d.hpp"

#include <arrow/api.h>

namespace rerun {
    namespace datatypes {
        const std::shared_ptr<arrow::DataType> &TranslationRotationScale3D::to_arrow_datatype() {
            static const auto datatype = arrow::struct_({
                arrow::field("translation", rerun::datatypes::Vec3D::to_arrow_datatype(), true),
                arrow::field("rotation", rerun::datatypes::Rotation3D::to_arrow_datatype(), true),
                arrow::field("scale", rerun::datatypes::Scale3D::to_arrow_datatype(), true),
                arrow::field("from_parent", arrow::boolean(), false),
            });
            return datatype;
        }

        arrow::Result<std::shared_ptr<arrow::StructBuilder>>
            TranslationRotationScale3D::new_arrow_array_builder(arrow::MemoryPool *memory_pool) {
            if (!memory_pool) {
                return arrow::Status::Invalid("Memory pool is null.");
            }

            return arrow::Result(std::make_shared<arrow::StructBuilder>(
                to_arrow_datatype(),
                memory_pool,
                std::vector<std::shared_ptr<arrow::ArrayBuilder>>({
                    rerun::datatypes::Vec3D::new_arrow_array_builder(memory_pool).ValueOrDie(),
                    rerun::datatypes::Rotation3D::new_arrow_array_builder(memory_pool).ValueOrDie(),
                    rerun::datatypes::Scale3D::new_arrow_array_builder(memory_pool).ValueOrDie(),
                    std::make_shared<arrow::BooleanBuilder>(memory_pool),
                })
            ));
        }

        arrow::Status TranslationRotationScale3D::fill_arrow_array_builder(
            arrow::StructBuilder *builder, const TranslationRotationScale3D *elements,
            size_t num_elements
        ) {
            if (!builder) {
                return arrow::Status::Invalid("Passed array builder is null.");
            }
            if (!elements) {
                return arrow::Status::Invalid("Cannot serialize null pointer to arrow array.");
            }

            {
                auto field_builder =
                    static_cast<arrow::FixedSizeListBuilder *>(builder->field_builder(0));
                ARROW_RETURN_NOT_OK(field_builder->Reserve(static_cast<int64_t>(num_elements)));
                for (size_t elem_idx = 0; elem_idx < num_elements; elem_idx += 1) {
                    const auto &element = elements[elem_idx];
                    if (element.translation.has_value()) {
                        ARROW_RETURN_NOT_OK(rerun::datatypes::Vec3D::fill_arrow_array_builder(
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
                    static_cast<arrow::DenseUnionBuilder *>(builder->field_builder(1));
                ARROW_RETURN_NOT_OK(field_builder->Reserve(static_cast<int64_t>(num_elements)));
                for (size_t elem_idx = 0; elem_idx < num_elements; elem_idx += 1) {
                    const auto &element = elements[elem_idx];
                    if (element.rotation.has_value()) {
                        ARROW_RETURN_NOT_OK(rerun::datatypes::Rotation3D::fill_arrow_array_builder(
                            field_builder,
                            &element.rotation.value(),
                            1
                        ));
                    } else {
                        ARROW_RETURN_NOT_OK(field_builder->AppendNull());
                    }
                }
            }
            {
                auto field_builder =
                    static_cast<arrow::DenseUnionBuilder *>(builder->field_builder(2));
                ARROW_RETURN_NOT_OK(field_builder->Reserve(static_cast<int64_t>(num_elements)));
                for (size_t elem_idx = 0; elem_idx < num_elements; elem_idx += 1) {
                    const auto &element = elements[elem_idx];
                    if (element.scale.has_value()) {
                        ARROW_RETURN_NOT_OK(rerun::datatypes::Scale3D::fill_arrow_array_builder(
                            field_builder,
                            &element.scale.value(),
                            1
                        ));
                    } else {
                        ARROW_RETURN_NOT_OK(field_builder->AppendNull());
                    }
                }
            }
            {
                auto field_builder =
                    static_cast<arrow::BooleanBuilder *>(builder->field_builder(3));
                ARROW_RETURN_NOT_OK(field_builder->Reserve(static_cast<int64_t>(num_elements)));
                for (size_t elem_idx = 0; elem_idx < num_elements; elem_idx += 1) {
                    ARROW_RETURN_NOT_OK(field_builder->Append(elements[elem_idx].from_parent));
                }
            }
            ARROW_RETURN_NOT_OK(builder->AppendValues(static_cast<int64_t>(num_elements), nullptr));

            return arrow::Status::OK();
        }
    } // namespace datatypes
} // namespace rerun
