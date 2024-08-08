// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/datatypes/color_model.fbs".

#include "color_model.hpp"

#include <arrow/builder.h>
#include <arrow/type_fwd.h>

namespace rerun {
    const std::shared_ptr<arrow::DataType>& Loggable<datatypes::ColorModel>::arrow_datatype() {
        static const auto datatype = arrow::uint8();
        return datatype;
    }

    Result<std::shared_ptr<arrow::Array>> Loggable<datatypes::ColorModel>::to_arrow(
        const datatypes::ColorModel* instances, size_t num_instances
    ) {
        // TODO(andreas): Allow configuring the memory pool.
        arrow::MemoryPool* pool = arrow::default_memory_pool();
        auto datatype = arrow_datatype();

        ARROW_ASSIGN_OR_RAISE(auto builder, arrow::MakeBuilder(datatype, pool))
        if (instances && num_instances > 0) {
            RR_RETURN_NOT_OK(Loggable<datatypes::ColorModel>::fill_arrow_array_builder(
                static_cast<arrow::UInt8Builder*>(builder.get()),
                instances,
                num_instances
            ));
        }
        std::shared_ptr<arrow::Array> array;
        ARROW_RETURN_NOT_OK(builder->Finish(&array));
        return array;
    }

    rerun::Error Loggable<datatypes::ColorModel>::fill_arrow_array_builder(
        arrow::UInt8Builder* builder, const datatypes::ColorModel* elements, size_t num_elements
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
            const auto variant = elements[elem_idx];
            ARROW_RETURN_NOT_OK(builder->Append(static_cast<uint8_t>(variant)));
        }

        return Error::ok();
    }
} // namespace rerun
