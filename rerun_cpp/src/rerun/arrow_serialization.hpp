#pragma once

#include <cstddef>
#include <memory>

#include "loggable.hpp"
#include "result.hpp"

// Implementation-only: do not include from public SDK headers.
namespace arrow {
    class Array;
    class ArrayBuilder;
    class DataType;
} // namespace arrow

namespace rerun::detail {
    using ArrowFill = Error (*)(arrow::ArrayBuilder*, const void*, size_t);

    // The datatype must create the builder expected by fill, and instances must point to
    // count objects of the type expected by fill. The callback is invoked at most once.
    // Null instances or zero count produce an empty array without invoking fill.
    Result<std::shared_ptr<arrow::Array>> serialize_arrow(
        const std::shared_ptr<arrow::DataType>& datatype, const void* instances, size_t count,
        ArrowFill fill
    );

    /// Adapts a Loggable's typed fill method to the shared builder lifecycle.
    /// Builder must match both Loggable<T>::arrow_data_type() and its fill method.
    /// Builder must be complete when this template is instantiated.
    template <typename Builder, typename T>
    Result<std::shared_ptr<arrow::Array>> serialize_arrow(const T* instances, size_t count) {
        return serialize_arrow(
            Loggable<T>::arrow_data_type(),
            instances,
            count,
            [](arrow::ArrayBuilder* builder, const void* elements, size_t num_elements) {
                return Loggable<T>::fill_arrow_array_builder(
                    static_cast<Builder*>(builder),
                    static_cast<const T*>(elements),
                    num_elements
                );
            }
        );
    }

    /// Checks that the array builder and input elements are non-null, even for an empty batch.
    /// Returns UnexpectedNullArgument for a null pointer, checking the builder first, or Ok otherwise.
    /// Does not dereference the pointers or check their types or available capacity.
    Error check_arrow_builder_arguments(const arrow::ArrayBuilder* builder, const void* elements);
} // namespace rerun::detail
