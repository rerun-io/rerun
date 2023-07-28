#pragma once

#include <cstddef> // size_t
#include <cstdint> // uint32_t etc
#include <vector>

#include "data_cell.hpp"

namespace rr {
    struct DataCell;

    enum StoreKind {
        Recording,
        Blueprint,
    };

    class RecordingStream {
      public:
        RecordingStream(
            const char* app_id, const char* addr, StoreKind store_kind = StoreKind::Recording
        );
        ~RecordingStream();

        /// Must be called first, if at all.
        static void init_global(const char* app_id, const char* addr);

        /// Access the global recording stream.
        /// Aborts if `init_global` has not yet been called.
        static RecordingStream global();

        /// Logs an archetype.
        ///
        /// Prefer this interface for ease of use over the more general `log_components` interface.
        template <typename T>
        void log_archetype(const char* entity_path, const T& archetype) {
            // TODO(andreas): Handle splats.
            // TODO(andreas): Error handling.
            const auto data_cells = archetype.to_data_cells().ValueOrDie();
            log_data_row(
                entity_path,
                archetype.num_instances(),
                data_cells.size(),
                data_cells.data()
            );
        }

        /// Logs a list of component arrays.
        ///
        /// This forms the "medium level API", for easy to use high level api, prefer `log` to log
        /// built-in archetypes.
        ///
        /// Expects component arrays in continuous memory in with a std::vector/std::array like
        /// interface, i.e. each component array needs a data & size method.
        ///
        /// TODO(andreas): More documentation, examples etc.
        /// TODO(andreas): Test with different array types - vector/array seem to work but we should
        /// also support C arrays.
        template <typename... Ts>
        void log_components(const char* entity_path, const Ts&... component_array) {
            // TODO(andreas): Handle splats.
            const size_t num_instances = size_of_first_collection(component_array...);

            std::vector<DataCell> data_cells;
            data_cells.reserve(sizeof...(Ts));
            (
                [&data_cells, &component_array] {
                    using ComponentType = std::remove_pointer_t<decltype(component_array.data())>;
                    const auto cell =
                        ComponentType::to_data_cell(component_array.data(), component_array.size())
                            .ValueOrDie(); // TODO(andreas): Error handling.
                    data_cells.push_back(cell);
                }(),
                ...
            );

            log_data_row(entity_path, num_instances, data_cells.size(), data_cells.data());
        }

        /// Low level API that logs raw data cells to the recording stream.
        ///
        /// I.e. logs a number of components arrays (each with a same number of instances) to a
        /// single entity path.
        void log_data_row(
            const char* entity_path, uint32_t num_instances, size_t num_data_cells,
            const DataCell* data_cells
        );

      private:
        /// Returns size of the first collection of a list of collections.
        template <typename First, typename... Ts>
        static size_t size_of_first_collection(const First& first, const Ts&... ts) {
            return first.size();
        }

        RecordingStream() : _id{0} {}

        RecordingStream(uint32_t id) : _id{id} {}

        uint32_t _id;

        static RecordingStream s_global;
    };
} // namespace rr
