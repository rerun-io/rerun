#include <catch2/catch_test_macros.hpp>

// TODO(andreas): These should be namespaced `rerun/recording_stream.hpp`
#include <archetypes/points2d.hpp>
#include <components/point2d.hpp>
#include <datatypes/point2d.hpp>
#include <recording_stream.hpp>

#include <array>
#include <vector>

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
        GIVEN("a new RecordingStream of kind" << kind) {
            rr::RecordingStream stream("test", kind);

            THEN("it does not crash on destruction") {}

            THEN("it reports the correct kind") {
                CHECK(stream.kind() == kind);
            }
        }
    }
}

SCENARIO("RecordingStream can be set as global and thread local", TEST_TAG) {
    for (auto kind : std::array{rr::StoreKind::Recording, rr::StoreKind::Blueprint}) {
        GIVEN("a store kind" << kind) {
            WHEN("querying the current one") {
                auto& stream = rr::RecordingStream::current(kind);

                THEN("it reports the correct kind") {
                    CHECK(stream.kind() == kind);
                }
            }

            WHEN("creating a new stream") {
                rr::RecordingStream stream("test", kind);

                THEN("it can be set as global") {
                    stream.set_global();
                }
                THEN("it can be set as thread local") {
                    stream.set_thread_local();
                }

                // TODO(andreas): There's no way of telling right now if the set stream is
                // functional.
            }
        }
    }
}

SCENARIO("RecordingStream can be used for logging archetypes and components", TEST_TAG) {
    for (auto kind : std::array{rr::StoreKind::Recording, rr::StoreKind::Blueprint}) {
        GIVEN("a store kind" << kind) {
            WHEN("creating a new stream") {
                rr::RecordingStream stream("test", kind);

                THEN("components as c-array can be logged") {
                    rr::components::Point2D c_style_array[2] = {
                        rr::datatypes::Point2D{1.0, 2.0},
                        rr::datatypes::Point2D{4.0, 5.0},
                    };

                    stream.log_components("as-carray", c_style_array);
                }
                THEN("components as std::array can be logged") {
                    stream.log_components(
                        "as-array",
                        std::array<rr::components::Point2D, 2>{
                            rr::datatypes::Point2D{1.0, 2.0},
                            rr::datatypes::Point2D{4.0, 5.0},
                        }
                    );
                }
                THEN("components as std::vector can be logged") {
                    stream.log_components(
                        "as-vector",
                        std::vector<rr::components::Point2D>{
                            rr::datatypes::Point2D{1.0, 2.0},
                            rr::datatypes::Point2D{4.0, 5.0},
                        }
                    );
                }
                THEN("several components with a mix of vector, array and c-array can be logged") {
                    rr::components::Label c_style_array[3] = {
                        rr::components::Label("hello"),
                        rr::components::Label("friend"),
                        rr::components::Label("yo"),
                    };
                    stream.log_components(
                        "as-mix",
                        std::vector{
                            rr::components::Point2D(rr::datatypes::Point2D{0.0, 0.0}),
                            rr::components::Point2D(rr::datatypes::Point2D{1.0, 3.0}),
                            rr::components::Point2D(rr::datatypes::Point2D{5.0, 5.0}),
                        },
                        std::array{
                            rr::components::Color(0xFF0000FF),
                            rr::components::Color(0x00FF00FF),
                            rr::components::Color(0x0000FFFF),
                        },
                        c_style_array
                    );
                }

                THEN("an archetype can be logged") {
                    stream.log_archetype(
                        "3d/points",
                        rr::archetypes::Points2D({
                            rr::datatypes::Point2D{1.0, 2.0},
                            rr::datatypes::Point2D{4.0, 5.0},
                        })
                    );
                }

                // TODO(andreas): There's no way of telling right now if the set stream is
                // functional and where those messages went.
            }
        }
    }
}

// TODO: save and connect
