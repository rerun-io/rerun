#include "../error.hpp"
#include "tensor.hpp"

#include <algorithm> // std::min
#include <string>    // std::to_string
#include <utility>   // std::move

#include <arrow/array/array_binary.h>
#include <arrow/array/array_nested.h>
#include <arrow/builder.h>

#include "../collection_adapter_builtins.hpp"

namespace rerun::archetypes {

#if 0
    // <CODEGEN_COPY_TO_HEADER>

RR_DISABLE_MAYBE_UNINITIALIZED_PUSH

    /// New Tensor from dimensions and tensor buffer.
    Tensor(Collection<uint64_t> shape, datatypes::TensorBuffer buffer)
        : Tensor(datatypes::TensorData(std::move(shape), std::move(buffer))) {}

RR_DISABLE_MAYBE_UNINITIALIZED_POP

    /// New tensor from dimensions and pointer to tensor data.
    ///
    /// Type must be one of the types supported by `rerun::datatypes::TensorData`.
    /// \param shape
    /// Shape of the image. Determines the number of elements expected to be in `data`.
    /// \param data_
    /// Target of the pointer must outlive the archetype.
    template <typename TElement>
    explicit Tensor(Collection<uint64_t> shape, const TElement* data_)
        : Tensor(datatypes::TensorData(std::move(shape), data_)) {}

    /// Update the `names` of the contained `TensorData` dimensions.
    ///
    /// Any existing Dimension names will be overwritten.
    ///
    /// If too many, or too few names are provided, this function will call
    /// Error::handle and then proceed to only update the subset of names that it can.
    Tensor with_dim_names(Collection<std::string> names) &&;

    // </CODEGEN_COPY_TO_HEADER>
#endif

    Result<std::shared_ptr<arrow::Array>> tensor_data_with_dim_names(
        const std::optional<rerun::ComponentBatch>& data, Collection<std::string> names
    ) {
        if (names.empty()) {
            return std::move(data.value().array);
        }
        if (!data.has_value()) {
            return Error(
                ErrorCode::InvalidComponent,
                "Can't set names on a tensor that doesn't have any data"
            );
        }

        // TODO(#6832): Right now everything is crammed into a single struct array,
        // so we have to essentially take this struct apart, come up with a new `names` field and
        // put it back together.
        // See also `tensor_data.cpp`.

        auto data_struct_array = std::dynamic_pointer_cast<arrow::StructArray>(data.value().array);
        if (!data_struct_array) {
            return Error(ErrorCode::InvalidComponent, "Tensor data is not a struct array");
        }
        if (data_struct_array->length() == 0) {
            return Error(
                ErrorCode::InvalidComponent,
                "Can't set names on a tensor that doesn't have any data"
            );
        }
        if (data_struct_array->length() > 1) {
            return Error(
                ErrorCode::InvalidComponent,
                "Can't set dimension names on a tensor archetype with multiple tensor data instances."
            );
        }

        auto buffer_array = data_struct_array->GetFieldByName("buffer");
        if (!buffer_array) {
            return Error(
                ErrorCode::InvalidComponent,
                "Tensor's data array doesn't have a buffer field"
            );
        }
        auto shape_list_array =
            std::dynamic_pointer_cast<arrow::ListArray>(data_struct_array->GetFieldByName("shape"));
        if (!shape_list_array) {
            return Error(
                ErrorCode::InvalidComponent,
                "Tensor's data array doesn't have a shape list array field"
            );
        }

        if (shape_list_array->values()->length() != static_cast<int64_t>(names.size())) {
            return Error(
                ErrorCode::InvalidTensorDimension,
                "Wrong number of names provided for tensor dimension. " +
                    std::to_string(names.size()) + " provided but " +
                    std::to_string(shape_list_array->values()->length()) + " expected."
            );
        }

        // Build a new names array and put everything back together.
        auto datatype = rerun::Loggable<rerun::datatypes::TensorData>::arrow_datatype();
        auto name_field = datatype->field(1);
        arrow::MemoryPool* pool = arrow::default_memory_pool();
        ARROW_ASSIGN_OR_RAISE(auto names_builder, arrow::MakeBuilder(name_field->type(), pool))
        auto names_list_builder = static_cast<arrow::ListBuilder*>(names_builder.get());

        ARROW_RETURN_NOT_OK(names_list_builder->Append());
        auto value_builder =
            static_cast<arrow::StringBuilder*>(names_list_builder->value_builder());
        ARROW_RETURN_NOT_OK(value_builder->Reserve(static_cast<int64_t>(names.size())));
        for (const auto& name : names) {
            ARROW_RETURN_NOT_OK(value_builder->Append(name));
        }
        ARROW_ASSIGN_OR_RAISE(auto names_list_array, names_list_builder->Finish())

        // wrap in `ARROW_RETURN_NOT_OK` instead of returning directly to do the conversion to rerun::Result.
        ARROW_ASSIGN_OR_RAISE(
            auto result,
            arrow::StructArray::Make(
                std::vector<std::shared_ptr<arrow::Array>>{
                    shape_list_array,
                    names_list_array,
                    buffer_array,
                },
                datatype->fields()
            )
        )

        return rerun::Result(std::static_pointer_cast<arrow::Array>(result));
    }

    Tensor Tensor::with_dim_names(Collection<std::string> names) && {
        auto result = tensor_data_with_dim_names(this->data, names);
        if (result.is_err()) {
            result.error.handle();
            return std::move(*this);
        }

        this->data.value().array = std::move(result.value);

        return std::move(*this);
    }

} // namespace rerun::archetypes
