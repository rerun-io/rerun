// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/datatypes/angle.fbs".

#include "angle.hpp"

#include <arrow/builder.h>
#include <arrow/type_fwd.h>

namespace rerun::datatypes {}

namespace rerun {
    const std::shared_ptr<arrow::DataType>& Loggable<datatypes::Angle>::arrow_datatype() {
        static const auto datatype = arrow::dense_union({
            arrow::field("_null_markers", arrow::null(), true, nullptr),
            arrow::field("Radians", arrow::float32(), false),
            arrow::field("Degrees", arrow::float32(), false),
        });
        return datatype;
    }

    rerun::Error Loggable<datatypes::Angle>::fill_arrow_array_builder(
        arrow::DenseUnionBuilder* builder, const datatypes::Angle* elements, size_t num_elements
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

        ARROW_RETURN_NOT_OK(builder->Reserve(static_cast<int64_t>(num_elements)));
        for (size_t elem_idx = 0; elem_idx < num_elements; elem_idx += 1) {
            const auto& union_instance = elements[elem_idx];
            ARROW_RETURN_NOT_OK(builder->Append(static_cast<int8_t>(union_instance.get_union_tag()))
            );

            auto variant_index = static_cast<int>(union_instance.get_union_tag());
            auto variant_builder_untyped = builder->child_builder(variant_index).get();

            using TagType = datatypes::detail::AngleTag;
            switch (union_instance.get_union_tag()) {
                case TagType::None: {
                    ARROW_RETURN_NOT_OK(variant_builder_untyped->AppendNull());
                } break;
                case TagType::Radians: {
                    auto variant_builder =
                        static_cast<arrow::FloatBuilder*>(variant_builder_untyped);
                    ARROW_RETURN_NOT_OK(
                        variant_builder->Append(union_instance.get_union_data().radians)
                    );
                } break;
                case TagType::Degrees: {
                    auto variant_builder =
                        static_cast<arrow::FloatBuilder*>(variant_builder_untyped);
                    ARROW_RETURN_NOT_OK(
                        variant_builder->Append(union_instance.get_union_data().degrees)
                    );
                } break;
            }
        }

        return Error::ok();
    }

    Result<std::shared_ptr<arrow::Array>> Loggable<datatypes::Angle>::to_arrow(
        const datatypes::Angle* instances, size_t num_instances
    ) {
        // TODO(andreas): Allow configuring the memory pool.
        arrow::MemoryPool* pool = arrow::default_memory_pool();
        auto datatype = arrow_datatype();

        ARROW_ASSIGN_OR_RAISE(auto builder, arrow::MakeBuilder(datatype, pool))
        if (instances && num_instances > 0) {
            RR_RETURN_NOT_OK(Loggable<datatypes::Angle>::fill_arrow_array_builder(
                static_cast<arrow::DenseUnionBuilder*>(builder.get()),
                instances,
                num_instances
            ));
        }
        std::shared_ptr<arrow::Array> array;
        ARROW_RETURN_NOT_OK(builder->Finish(&array));
        return array;
    }
} // namespace rerun
