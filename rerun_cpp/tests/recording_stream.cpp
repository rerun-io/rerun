#include <catch2/catch_test_macros.hpp>
#include <recording_stream.hpp> // TODO(andreas): These should be namespaced `rerun/recording_stream.hpp`

#include <array>

#define TEST_TAG "[recording_stream]"

namespace rr {
    std::ostream& operator<<(std::ostream& os, StoreKind kind) {
        switch (kind) {
            case rr::StoreKind::Recording:
                os << "StoreKind::Recording";
                break;
            case rr::StoreKind::Blueprint:
                os << "StoreKind::Blueprint";
                break;
            default:
                FAIL("Unknown StoreKind");
                break;
        }
        return os;
    }
} // namespace rr

SCENARIO("RecordingStream can be created, destroyed and lists correct properties", TEST_TAG) {
    for (auto kind : std::array{rr::StoreKind::Recording, rr::StoreKind::Blueprint}) {
        GIVEN("a new RecordingStream of kind " << kind) {
            rr::RecordingStream stream("test", kind);

            THEN("it does not crash on destruction") {}

            THEN("it reports the correct kind") {
                CHECK(stream.kind() == kind);
            }
        }
    }
}
