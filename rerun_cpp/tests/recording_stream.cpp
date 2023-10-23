#include <catch2/catch_test_macros.hpp>
#include <catch2/generators/catch_generators.hpp>

#pragma GCC diagnostic push
#pragma GCC diagnostic ignored "-Wsign-conversion"
#include <arrow/buffer.h>
#pragma GCC diagnostic pop

#include "error_check.hpp"

#include <rerun/archetypes/points2d.hpp>
#include <rerun/datatypes/vec2d.hpp>
#include <rerun/recording_stream.hpp>

#include <array>
#include <filesystem>
#include <vector>

namespace fs = std::filesystem;
namespace rrc = rerun::components;

#define TEST_TAG "[recording_stream]"

struct BadComponent {
    static const char* NAME;
    static rerun::Error error;

    static rerun::Result<rerun::DataCell> to_data_cell(const BadComponent*, size_t) {
        return error;
    }
};

const char* BadComponent::NAME = "bad!";
rerun::Error BadComponent::error = rerun::Error(rerun::ErrorCode::Unknown, "BadComponent");

struct BadArchetype {
    size_t num_instances() const {
        return 1;
    }
};

namespace rerun {
    template <>
    struct AsComponents<BadArchetype> {
        static rerun::Result<std::vector<rerun::SerializedComponentBatch>>
            serialize(const BadArchetype&) {
            return BadComponent::error;
        }
    };
} // namespace rerun

namespace rerun {
    std::ostream& operator<<(std::ostream& os, StoreKind kind) {
        switch (kind) {
            case rerun::StoreKind::Recording:
                os << "StoreKind::Recording";
                break;
            case rerun::StoreKind::Blueprint:
                os << "StoreKind::Blueprint";
                break;
            default:
                FAIL("Unknown StoreKind");
                break;
        }
        return os;
    }
} // namespace rerun

SCENARIO("RecordingStream can be created, destroyed and lists correct properties", TEST_TAG) {
    const auto kind = GENERATE(rerun::StoreKind::Recording, rerun::StoreKind::Blueprint);

    GIVEN("recording stream kind" << kind) {
        AND_GIVEN("a valid application id") {
            THEN("creating a new stream does not log an error") {
                rerun::RecordingStream stream = check_logged_error([&] {
                    return rerun::RecordingStream("rerun_example_test", kind);
                });

                AND_THEN("it does not crash on destruction") {}

                AND_THEN("it reports the correct kind") {
                    CHECK(stream.kind() == kind);
                }
            }
        }

        AND_GIVEN("a nullptr for the application id") {
            THEN("creating a new stream logs a null argument error") {
                check_logged_error(
                    [&] { rerun::RecordingStream stream(nullptr, kind); },
                    rerun::ErrorCode::UnexpectedNullArgument
                );
            }
        }
        AND_GIVEN("invalid utf8 character sequence for the application id") {
            THEN("creating a new stream logs an invalid string argument error") {
                check_logged_error(
                    [&] { rerun::RecordingStream stream("\xc3\x28", kind); },
                    rerun::ErrorCode::InvalidStringArgument
                );
            }
        }
    }
}

SCENARIO("RecordingStream can be set as global and thread local", TEST_TAG) {
    for (auto kind : std::array{rerun::StoreKind::Recording, rerun::StoreKind::Blueprint}) {
        GIVEN("a store kind" << kind) {
            WHEN("querying the current one") {
                auto& stream = rerun::RecordingStream::current(kind);

                THEN("it reports the correct kind") {
                    CHECK(stream.kind() == kind);
                }
            }

            WHEN("creating a new stream") {
                rerun::RecordingStream stream("test", kind);

                THEN("it can be set as global") {
                    stream.set_global();
                }
                // TODO(#2889): Setting thread locals causes a crash on shutdown on macOS.
#ifndef __APPLE__
                THEN("it can be set as thread local") {
                    stream.set_thread_local();
                }
#endif

                // TODO(andreas): There's no way of telling right now if the set stream is
                // functional.
            }
        }
    }
}

SCENARIO("RecordingStream can be used for logging archetypes and components", TEST_TAG) {
    for (auto kind : std::array{rerun::StoreKind::Recording, rerun::StoreKind::Blueprint}) {
        GIVEN("a store kind" << kind) {
            WHEN("creating a new stream") {
                rerun::RecordingStream stream("test", kind);

                // We can make single components work, but this would make error messages a lot
                // worse since we'd have to implement the base `AsComponents` template for this.
                //
                // THEN("single components can be logged") {
                //     stream.log(
                //         "single-components",
                //         rrc::Position2D{1.0, 2.0},
                //         rrc::Color(0x00FF00FF)
                //     );
                // }

                THEN("components as c-array can be logged") {
                    rrc::Position2D c_style_array[2] = {
                        rerun::datatypes::Vec2D{1.0, 2.0},
                        rerun::datatypes::Vec2D{4.0, 5.0},
                    };

                    stream.log("as-carray", c_style_array);
                    stream.log_timeless("as-carray", c_style_array);
                }
                THEN("components as std::initializer_list can be logged") {
                    const auto c_style_array = {
                        rerun::components::Position2D{1.0, 2.0},
                        rerun::components::Position2D{4.0, 5.0},
                    };
                    stream.log("as-initializer-list", c_style_array);
                    stream.log_timeless("as-initializer-list", c_style_array);
                }
                THEN("components as std::array can be logged") {
                    stream.log(
                        "as-array",
                        std::array<rrc::Position2D, 2>{
                            rerun::datatypes::Vec2D{1.0, 2.0},
                            rerun::datatypes::Vec2D{4.0, 5.0},
                        }
                    );
                    stream.log_timeless(
                        "as-array",
                        std::array<rrc::Position2D, 2>{
                            rerun::datatypes::Vec2D{1.0, 2.0},
                            rerun::datatypes::Vec2D{4.0, 5.0},
                        }
                    );
                }
                THEN("components as std::vector can be logged") {
                    stream.log(
                        "as-vector",
                        std::vector<rrc::Position2D>{
                            rerun::datatypes::Vec2D{1.0, 2.0},
                            rerun::datatypes::Vec2D{4.0, 5.0},
                        }
                    );
                    stream.log_timeless(
                        "as-vector",
                        std::vector<rrc::Position2D>{
                            rerun::datatypes::Vec2D{1.0, 2.0},
                            rerun::datatypes::Vec2D{4.0, 5.0},
                        }
                    );
                }
                THEN("several components with a mix of vector, array and c-array can be logged") {
                    rrc::Text c_style_array[3] = {
                        rrc::Text("hello"),
                        rrc::Text("friend"),
                        rrc::Text("yo"),
                    };
                    stream.log(
                        "as-mix",
                        std::vector{
                            rrc::Position2D(rerun::datatypes::Vec2D{0.0, 0.0}),
                            rrc::Position2D(rerun::datatypes::Vec2D{1.0, 3.0}),
                            rrc::Position2D(rerun::datatypes::Vec2D{5.0, 5.0}),
                        },
                        std::array{
                            rrc::Color(0xFF0000FF),
                            rrc::Color(0x00FF00FF),
                            rrc::Color(0x0000FFFF),
                        },
                        c_style_array
                    );
                    stream.log_timeless(
                        "as-mix",
                        std::vector{
                            rrc::Position2D(rerun::datatypes::Vec2D{0.0, 0.0}),
                            rrc::Position2D(rerun::datatypes::Vec2D{1.0, 3.0}),
                            rrc::Position2D(rerun::datatypes::Vec2D{5.0, 5.0}),
                        },
                        std::array{
                            rrc::Color(0xFF0000FF),
                            rrc::Color(0x00FF00FF),
                            rrc::Color(0x0000FFFF),
                        },
                        c_style_array
                    );
                }

                THEN("components with splatting some of them can be logged") {
                    stream.log(
                        "log-splat",
                        std::vector{
                            rrc::Position2D(rerun::datatypes::Vec2D{0.0, 0.0}),
                            rrc::Position2D(rerun::datatypes::Vec2D{1.0, 3.0}),
                        },
                        std::array{rrc::Color(0xFF0000FF)}
                    );
                    stream.log_timeless(
                        "log-splat",
                        std::vector{
                            rrc::Position2D(rerun::datatypes::Vec2D{0.0, 0.0}),
                            rrc::Position2D(rerun::datatypes::Vec2D{1.0, 3.0}),
                        },
                        std::array{rrc::Color(0xFF0000FF)}
                    );
                }

                THEN("an archetype can be logged") {
                    stream.log(
                        "log_archetype-splat",
                        rerun::archetypes::Points2D(
                            {rerun::datatypes::Vec2D{1.0, 2.0}, rerun::datatypes::Vec2D{4.0, 5.0}}
                        ).with_colors(rrc::Color(0xFF0000FF))
                    );
                    stream.log_timeless(
                        "log_archetype-splat",
                        rerun::archetypes::Points2D(
                            {rerun::datatypes::Vec2D{1.0, 2.0}, rerun::datatypes::Vec2D{4.0, 5.0}}
                        ).with_colors(rrc::Color(0xFF0000FF))
                    );
                }

                // TODO(andreas): There's no way of telling right now if the set stream is
                // functional and where those messages went.
            }
        }
    }
}

SCENARIO("RecordingStream can log to file", TEST_TAG) {
    const char* test_path = "build/test_output";
    fs::create_directories(test_path);

    std::string test_rrd0 = std::string(test_path) + "test-file-0.rrd";
    std::string test_rrd1 = std::string(test_path) + "test-file-1.rrd";

    fs::remove(test_rrd0.c_str());
    fs::remove(test_rrd1.c_str());

    GIVEN("a new RecordingStream") {
        auto stream0 = std::make_unique<rerun::RecordingStream>("test");

        AND_GIVEN("a nullptr for the save path") {
            THEN("then the save call returns a null argument error") {
                CHECK(stream0->save(nullptr).code == rerun::ErrorCode::UnexpectedNullArgument);
            }
        }
        AND_GIVEN("valid save path " << test_rrd0) {
            AND_GIVEN("a directory already existing at this path") {
                fs::create_directory(test_rrd0.c_str());
                THEN("then the save call fails") {
                    CHECK(
                        stream0->save(test_rrd0.c_str()).code ==
                        rerun::ErrorCode::RecordingStreamSaveFailure
                    );
                }
            }
            THEN("save call returns no error") {
                REQUIRE(stream0->save(test_rrd0.c_str()).is_ok());

                THEN("a new file got immediately created") {
                    CHECK(fs::exists(test_rrd0));
                }

                WHEN("creating a second stream") {
                    auto stream1 = std::make_unique<rerun::RecordingStream>("test2");

                    WHEN("saving that one to a different file " << test_rrd1) {
                        REQUIRE(stream1->save(test_rrd1.c_str()).is_ok());

                        WHEN("logging a component to the second stream") {
                            check_logged_error([&] {
                                stream1->log(
                                    "as-array",
                                    std::array<rrc::Position2D, 2>{
                                        rerun::datatypes::Vec2D{1.0, 2.0},
                                        rerun::datatypes::Vec2D{4.0, 5.0},
                                    }
                                );
                            });

                            THEN("after destruction, the second stream produced a bigger file") {
                                stream0.reset();
                                stream1.reset();
                                CHECK(fs::file_size(test_rrd0) < fs::file_size(test_rrd1));
                            }
                        }
                        WHEN("logging an archetype to the second stream") {
                            check_logged_error([&] {
                                stream1->log(
                                    "archetype",
                                    rerun::archetypes::Points2D({
                                        rerun::datatypes::Vec2D{1.0, 2.0},
                                        rerun::datatypes::Vec2D{4.0, 5.0},
                                    })
                                );
                            });

                            THEN("after destruction, the second stream produced a bigger file") {
                                stream0.reset();
                                stream1.reset();
                                CHECK(fs::file_size(test_rrd0) < fs::file_size(test_rrd1));
                            }
                        }
                    }
                }
            }
        }
    }
}

void test_logging_to_connection(const char* address, rerun::RecordingStream& stream) {
    AND_GIVEN("a nullptr for the socket address") {
        THEN("then the connect call returns a null argument error") {
            CHECK(stream.connect(nullptr, 0.0f).code == rerun::ErrorCode::UnexpectedNullArgument);
        }
    }
    AND_GIVEN("an invalid address for the socket address") {
        THEN("then the save call fails") {
            CHECK(
                stream.connect("definitely not valid!", 0.0f).code ==
                rerun::ErrorCode::InvalidSocketAddress
            );
        }
    }
    AND_GIVEN("a valid socket address " << address) {
        THEN("save call with zero timeout returns no error") {
            REQUIRE(stream.connect(address, 0.0f).is_ok());

            WHEN("logging a component and then flushing") {
                check_logged_error([&] {
                    stream.log(
                        "as-array",
                        std::array<rrc::Position2D, 2>{
                            rerun::datatypes::Vec2D{1.0, 2.0},
                            rerun::datatypes::Vec2D{4.0, 5.0},
                        }
                    );
                });
                stream.flush_blocking();

                THEN("does not crash") {
                    // No easy way to see if it got sent.
                }
            }
            WHEN("logging an archetype and then flushing") {
                check_logged_error([&] {
                    stream.log(
                        "archetype",
                        rerun::archetypes::Points2D({
                            rerun::datatypes::Vec2D{1.0, 2.0},
                            rerun::datatypes::Vec2D{4.0, 5.0},
                        })
                    );
                });

                stream.flush_blocking();

                THEN("does not crash") {
                    // No easy way to see if it got sent.
                }
            }
        }
    }
}

SCENARIO("RecordingStream can connect", TEST_TAG) {
    const char* address = "127.0.0.1:9876";
    GIVEN("a new RecordingStream") {
        rerun::RecordingStream stream("test-local");
        test_logging_to_connection(address, stream);
    }
    WHEN("setting a global RecordingStream and then discarding it") {
        {
            rerun::RecordingStream stream("test-global");
            stream.set_global();
        }
        GIVEN("the current recording stream") {
            test_logging_to_connection(address, rerun::RecordingStream::current());
        }
    }
}

SCENARIO("Recording stream handles invalid logging gracefully", TEST_TAG) {
    GIVEN("a new RecordingStream") {
        rerun::RecordingStream stream("test");

        AND_GIVEN("an invalid path") {
            auto variant = GENERATE(table<const char*, rerun::ErrorCode>({
                std::tuple<const char*, rerun::ErrorCode>(
                    nullptr,
                    rerun::ErrorCode::UnexpectedNullArgument
                ),
            }));
            const auto [path, error] = variant;
            auto v = rrc::Position2D{1.0, 2.0};

            THEN("try_log_data_row returns the correct error") {
                CHECK(stream.try_log_data_row(path, 0, 0, nullptr, true).code == error);
            }
            THEN("try_log returns the correct error") {
                CHECK(stream.try_log(path, rerun::archetypes::Points2D(v)).code == error);
            }
            THEN("log logs the correct error") {
                check_logged_error(
                    [&] { stream.log(std::get<0>(variant), rerun::archetypes::Points2D(v)); },
                    error
                );
            }
            THEN("try_log_timeless returns the correct error") {
                CHECK(stream.try_log_timeless(path, rerun::archetypes::Points2D(v)).code == error);
            }
            THEN("log_timeless logs the correct error") {
                check_logged_error(
                    [&] {
                        stream.log_timeless(std::get<0>(variant), rerun::archetypes::Points2D(v));
                    },
                    error
                );
            }
        }

        AND_GIVEN("a valid path") {
            const char* path = "valid";

            AND_GIVEN("a cell with a null buffer") {
                rerun::DataCell cell;
                cell.buffer = nullptr;
                cell.component_name = "valid";

                THEN("try_log_data_row fails with UnexpectedNullArgument") {
                    CHECK(
                        stream.try_log_data_row(path, 1, 1, &cell, true).code ==
                        rerun::ErrorCode::UnexpectedNullArgument
                    );
                }
            }

            AND_GIVEN("a cell with a null component name") {
                rerun::DataCell cell;
                cell.buffer = std::make_shared<arrow::Buffer>(nullptr, 0);
                cell.component_name = nullptr;

                THEN("try_log_data_row fails with UnexpectedNullArgument") {
                    CHECK(
                        stream.try_log_data_row(path, 1, 1, &cell, true).code ==
                        rerun::ErrorCode::UnexpectedNullArgument
                    );
                }
            }

            AND_GIVEN("a cell with a valid component name but invalid data") {
                uint8_t invalid_data[1] = {0};
                rerun::DataCell cell;
                cell.component_name = "very-valid";
                cell.buffer = std::make_shared<arrow::Buffer>(invalid_data, sizeof(invalid_data));

                THEN("try_log_data_row fails with ArrowIpcMessageParsingFailure") {
                    CHECK(
                        stream.try_log_data_row(path, 1, 1, &cell, true).code ==
                        rerun::ErrorCode::ArrowIpcMessageParsingFailure
                    );
                }
            }

            // TODO(andreas): Missing test that provokes `ArrowDataCellError`. It's fairly hard to
            // get there which I reckon is a good thing!
        }
    }
}

SCENARIO("Recording stream handles serialization failure during logging gracefully", TEST_TAG) {
    GIVEN("a new RecordingStream and a valid entity path") {
        rerun::RecordingStream stream("test");
        const char* path = "valid";
        AND_GIVEN("an component that fails serialization") {
            const auto component = BadComponent();
            BadComponent::error.code =
                GENERATE(rerun::ErrorCode::Unknown, rerun::ErrorCode::ArrowStatusCode_TypeError);

            THEN("calling log_component_batch with an array logs the serialization error") {
                check_logged_error(
                    [&] {
                        stream.log(path, std::array{component, component});
                    },
                    component.error.code
                );
            }
            THEN("calling log_component_batch with a vector logs the serialization error") {
                check_logged_error(
                    [&] {
                        stream.log(path, std::vector{component, component});
                    },
                    component.error.code
                );
            }
            THEN("calling log_component_batch with a c array logs the serialization error") {
                const BadComponent components[] = {component, component};
                check_logged_error([&] { stream.log(path, components); }, component.error.code);
            }
            THEN("calling try_log_component_batch with an array forwards the serialization error") {
                CHECK(stream.try_log(path, std::array{component, component}) == component.error);
            }
            THEN("calling try_log_component_batch with a vector forwards the serialization error") {
                CHECK(stream.try_log(path, std::vector{component, component}) == component.error);
            }
            THEN("calling try_log_component_batch with a c array forwards the serialization error"
            ) {
                const BadComponent components[] = {component, component};
                CHECK(stream.try_log(path, components) == component.error);
            }
        }
        AND_GIVEN("an archetype that fails serialization") {
            auto archetype = BadArchetype();
            BadComponent::error.code =
                GENERATE(rerun::ErrorCode::Unknown, rerun::ErrorCode::ArrowStatusCode_TypeError);

            THEN("calling log_archetype logs the serialization error") {
                check_logged_error([&] { stream.log(path, archetype); }, BadComponent::error.code);
            }
            THEN("calling log_archetype forwards the serialization error") {
                CHECK(stream.try_log(path, archetype) == BadComponent::error);
            }
        }
    }
}

SCENARIO("RecordingStream can set time without errors", TEST_TAG) {
    rerun::RecordingStream stream("test");

    SECTION("Setting time sequence does not log errors") {
        check_logged_error([&] { stream.set_time_sequence("my sequence", 1); });
    }
    SECTION("Setting time seconds does not log errors") {
        check_logged_error([&] { stream.set_time_seconds("my sequence", 1.0); });
    }
    SECTION("Setting time nanos does not log errors") {
        check_logged_error([&] { stream.set_time_nanos("my sequence", 1); });
    }
    SECTION("Resetting time does not log errors") {
        check_logged_error([&] { stream.reset_time(); });
    }

    SECTION("Disabling timeline does not log errors") {
        check_logged_error([&] { stream.disable_timeline("doesn't exist"); });
        check_logged_error([&] { stream.set_time_sequence("exists!", 123); });
        check_logged_error([&] { stream.disable_timeline("exists"); });
    }
}
