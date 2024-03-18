// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/blueprint/components/row_share.fbs".

#include "row_share.hpp"

#include <arrow/builder.h>
#include <arrow/type_fwd.h>

namespace rerun::blueprint::components {}

namespace rerun {
    const std::shared_ptr<arrow::DataType>&
        Loggable<blueprint::components::RowShare>::arrow_datatype() {
        static const auto datatype = arrow::float32();
        return datatype;
    }

    Result<std::shared_ptr<arrow::Array>> Loggable<blueprint::components::RowShare>::to_arrow(
        const blueprint::components::RowShare* instances, size_t num_instances
    ) {
        // TODO(andreas): Allow configuring the memory pool.
        arrow::MemoryPool* pool = arrow::default_memory_pool();
        auto datatype = arrow_datatype();

        ARROW_ASSIGN_OR_RAISE(auto builder, arrow::MakeBuilder(datatype, pool))
        if (instances && num_instances > 0) {
            RR_RETURN_NOT_OK(Loggable<blueprint::components::RowShare>::fill_arrow_array_builder(
                static_cast<arrow::FloatBuilder*>(builder.get()),
                instances,
                num_instances
            ));
        }
        std::shared_ptr<arrow::Array> array;
        ARROW_RETURN_NOT_OK(builder->Finish(&array));
        return array;
    }

    rerun::Error Loggable<blueprint::components::RowShare>::fill_arrow_array_builder(
        arrow::FloatBuilder* builder, const blueprint::components::RowShare* elements,
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

        static_assert(sizeof(*elements) == sizeof(elements->share));
        ARROW_RETURN_NOT_OK(
            builder->AppendValues(&elements->share, static_cast<int64_t>(num_elements))
        );

        return Error::ok();
    }
} // namespace rerun
