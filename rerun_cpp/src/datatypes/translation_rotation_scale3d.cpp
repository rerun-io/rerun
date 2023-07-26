// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.
// Based on "crates/re_types/definitions/rerun/datatypes/translation_rotation_scale3d.fbs"

#include "translation_rotation_scale3d.hpp"

#include "../datatypes/rotation3d.hpp"
#include "../datatypes/scale3d.hpp"
#include "../datatypes/vec3d.hpp"

#include <arrow/api.h>

namespace rr {
    namespace datatypes {
        std::shared_ptr<arrow::DataType> TranslationRotationScale3D::to_arrow_datatype() {
            return arrow::struct_({
                arrow::field(
                    "translation",
                    rr::datatypes::Vec3D::to_arrow_datatype(),
                    true,
                    nullptr
                ),
                arrow::field(
                    "rotation",
                    rr::datatypes::Rotation3D::to_arrow_datatype(),
                    true,
                    nullptr
                ),
                arrow::field("scale", rr::datatypes::Scale3D::to_arrow_datatype(), true, nullptr),
                arrow::field("from_parent", arrow::boolean(), false, nullptr),
            });
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
                    rr::datatypes::Vec3D::new_arrow_array_builder(memory_pool).ValueOrDie(),
                    rr::datatypes::Rotation3D::new_arrow_array_builder(memory_pool).ValueOrDie(),
                    rr::datatypes::Scale3D::new_arrow_array_builder(memory_pool).ValueOrDie(),
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

            return arrow::Status::NotImplemented(
                "TODO(andreas): extensions in structs are not yet supported"
            );
            return arrow::Status::NotImplemented(
                "TODO(andreas): extensions in structs are not yet supported"
            );
            return arrow::Status::NotImplemented(
                "TODO(andreas): extensions in structs are not yet supported"
            );
            {
                auto element_builder =
                    static_cast<arrow::BooleanBuilder *>(builder->field_builder(3));
                ARROW_RETURN_NOT_OK(element_builder->Reserve(num_elements));
                for (size_t elem_idx = 0; elem_idx < num_elements; elem_idx += 1) {
                    ARROW_RETURN_NOT_OK(element_builder->Append(elements[elem_idx].from_parent));
                }
            }
            ARROW_RETURN_NOT_OK(builder->AppendValues(num_elements, nullptr));

            return arrow::Status::OK();
        }
    } // namespace datatypes
} // namespace rr
