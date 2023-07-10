#pragma once

#include <cstddef> // size_t
#include <cstdint> // uint32_t etc

namespace rr {
    enum StoreKind {
        Recording,
        Blueprint,
    };

    struct DataCell {
        const char* component_name;
        size_t num_bytes;
        const uint8_t* bytes;
    };

    class RecordingStream {
      public:
        RecordingStream(const char* app_id, const char* addr,
                        StoreKind store_kind = StoreKind::Recording);
        ~RecordingStream();

        /// Must be called first, if at all.
        static void init_global(const char* app_id, const char* addr);

        /// Access the global recording stream.
        /// Aborts if `init_global` has not yet been called.
        static RecordingStream global();

        void log_data_row(const char* entity_path, uint32_t num_instances, size_t num_data_cells,
                          const DataCell* data_cells);

      private:
        RecordingStream() : _id{0} {}
        RecordingStream(uint32_t id) : _id{id} {}

        uint32_t _id;

        static RecordingStream s_global;
    };
} // namespace rr
