#pragma once

#include <memory> // shared_ptr

#include "component_type.hpp"
#include "error.hpp"

namespace arrow {
    class Array;
    class DataType;
} // namespace arrow

struct rr_data_cell;

namespace rerun {
    /// Arrow-encoded data of a single batch components for a single entity.
    ///
    /// Note that the DataCell doesn't own `datatype` and `component_name`.
    struct DataCell {
        /// How many instances of the component were serialized in this data cell.
        ///
        /// TODO(andreas): Just like in Rust, make this part of `AsComponents`.
        ///                 This will requiring inlining some things on RecordingStream and have some refactor ripples.
        ///                 But it's worth keeping the language bindings more similar!
        size_t num_instances;

        /// Arrow-encoded data of the component instances.
        std::shared_ptr<arrow::Array> array;

        /// The type of the component instances in array.
        ComponentTypeHandle component_type;

        /// To rerun C API data cell.
        ///
        /// The resulting `rr_data_cell` keeps the `arrow::Array` alive until it is released.
        Error to_c_ffi_struct(rr_data_cell& out_cell) const;
    };
} // namespace rerun
