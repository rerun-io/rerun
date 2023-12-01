#pragma once

#include <memory> // shared_ptr

#include "collection.hpp"
#include "component_type.hpp"
#include "error.hpp"
#include "loggable.hpp"

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

      public:
        /// Creates a new data cell from a collection of component instances.
        ///
        /// Automatically registers the component type the first time this type is encountered.
        template <typename T>
        static Result<DataCell> from_loggable(const rerun::Collection<T>& components) {
            static_assert(
                rerun::is_loggable<T>,
                "The given type does not implement the rerun::Loggable trait."
            );

            // Register type, only done once per type (but error check happens every time).
            static const Result<ComponentTypeHandle> component_type =
                ComponentType(Loggable<T>::Name, Loggable<T>::arrow_datatype())
                    .register_component();
            RR_RETURN_NOT_OK(component_type.error);

            /// TODO(#4257) should take a rerun::Collection instead of pointer and size.
            auto array = Loggable<T>::to_arrow(components.data(), components.size());
            RR_RETURN_NOT_OK(array.error);

            DataCell cell;
            cell.num_instances = components.size();
            cell.array = std::move(array.value);
            cell.component_type = component_type.value;
            return cell;
        }

        /// Creates a new data cell from a single component instance.
        ///
        /// Automatically registers the component type the first time this type is encountered.
        template <typename T>
        static Result<DataCell> from_loggable(const T& component) {
            // Collection adapter will automatically borrow for single elements, but let's do this explicitly, avoiding the extra hoop.
            const auto collection = Collection<T>::borrow(&component, 1);
            return from_loggable(collection);
        }

        /// To rerun C API data cell.
        ///
        /// The resulting `rr_data_cell` keeps the `arrow::Array` alive until it is released.
        Error to_c_ffi_struct(rr_data_cell& out_cell) const;
    };
} // namespace rerun
