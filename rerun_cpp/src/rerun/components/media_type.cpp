// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/components/media_type.fbs".

#include "media_type.hpp"

#include "../datatypes/utf8.hpp"

#include <arrow/builder.h>
#include <arrow/type_fwd.h>

namespace rerun::components {
    const char MediaType::NAME[] = "rerun.components.MediaType";

    const std::shared_ptr<arrow::DataType>& MediaType::arrow_datatype() {
        static const auto datatype = rerun::datatypes::Utf8::arrow_datatype();
        return datatype;
    }

    rerun::Error MediaType::fill_arrow_array_builder(
        arrow::StringBuilder* builder, const MediaType* elements, size_t num_elements
    ) {
        static_assert(sizeof(rerun::datatypes::Utf8) == sizeof(MediaType));
        RR_RETURN_NOT_OK(rerun::datatypes::Utf8::fill_arrow_array_builder(
            builder,
            reinterpret_cast<const rerun::datatypes::Utf8*>(elements),
            num_elements
        ));

        return Error::ok();
    }

    Result<rerun::DataCell> MediaType::to_data_cell(
        const MediaType* instances, size_t num_instances
    ) {
        // TODO(andreas): Allow configuring the memory pool.
        arrow::MemoryPool* pool = arrow::default_memory_pool();

        ARROW_ASSIGN_OR_RAISE(auto builder, arrow::MakeBuilder(arrow_datatype(), pool))
        if (instances && num_instances > 0) {
            RR_RETURN_NOT_OK(MediaType::fill_arrow_array_builder(
                static_cast<arrow::StringBuilder*>(builder.get()),
                instances,
                num_instances
            ));
        }
        std::shared_ptr<arrow::Array> array;
        ARROW_RETURN_NOT_OK(builder->Finish(&array));

        return rerun::DataCell::create(
            MediaType::NAME,
            MediaType::arrow_datatype(),
            std::move(array)
        );
    }
} // namespace rerun::components
