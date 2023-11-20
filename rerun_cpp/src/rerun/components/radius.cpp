// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/components/radius.fbs".

#include "radius.hpp"

#include <arrow/builder.h>
#include <arrow/type_fwd.h>

namespace rerun::components {
    const char Radius::NAME[] = "rerun.components.Radius";

    const std::shared_ptr<arrow::DataType>& Radius::arrow_datatype() {
        static const auto datatype = arrow::float32();
        return datatype;
    }

    rerun::Error Radius::fill_arrow_array_builder(
        arrow::FloatBuilder* builder, const Radius* elements, size_t num_elements
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

        static_assert(sizeof(*elements) == sizeof(elements->value));
        ARROW_RETURN_NOT_OK(
            builder->AppendValues(&elements->value, static_cast<int64_t>(num_elements))
        );

        return Error::ok();
    }

    Result<rerun::DataCell> Radius::to_data_cell(const Radius* instances, size_t num_instances) {
        // TODO(andreas): Allow configuring the memory pool.
        arrow::MemoryPool* pool = arrow::default_memory_pool();

        ARROW_ASSIGN_OR_RAISE(auto builder, arrow::MakeBuilder(arrow_datatype(), pool));
        if (instances && num_instances > 0) {
            RR_RETURN_NOT_OK(Radius::fill_arrow_array_builder(
                static_cast<arrow::FloatBuilder*>(builder.get()),
                instances,
                num_instances
            ));
        }
        std::shared_ptr<arrow::Array> array;
        ARROW_RETURN_NOT_OK(builder->Finish(&array));

        return rerun::DataCell::create(Radius::NAME, Radius::arrow_datatype(), std::move(array));
    }
} // namespace rerun::components
