#pragma once

#include <cstddef> // size_t
#include <cstdint> // uint32_t etc

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

        // TODO: docs

        // template <typename T>
        // void log(const char* entity_path, const T& archetype) {
        //     log_archetype(entity_path, archetype);
        // }

        // template <typename T>
        // void log_archetype(const char* entity_path, const T& archetype) {
        //     // TODO:
        // }

        // template <typename T>
        // void log_components(
        //     const char* entity_path, const std::vector<T>* component_arrays, size_t
        //     num_components
        // ) {
        //     // TODO:
        // }

        /// Low level API that logs raw data cells to the recording stream.
        ///
        /// I.e. logs a number of components arrays (each with a same number of instances) to a
        /// single entity path.
        void log_data_row(
            const char* entity_path, uint32_t num_instances, size_t num_data_cells,
            const DataCell* data_cells
        );

      private:
        RecordingStream() : _id{0} {}

        RecordingStream(uint32_t id) : _id{id} {}

        uint32_t _id;

        static RecordingStream s_global;
    };
} // namespace rr
