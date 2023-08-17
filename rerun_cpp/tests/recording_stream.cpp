#include <catch2/catch_test_macros.hpp>
#include <catch2/generators/catch_generators.hpp>

#pragma GCC diagnostic push
#pragma GCC diagnostic ignored "-Wsign-conversion"
#include <arrow/buffer.h>
#pragma GCC diagnostic pop

#include "error_check.hpp"

#include <rerun/archetypes/points2d.hpp>
#include <rerun/components/point2d.hpp>
#include <rerun/datatypes/vec2d.hpp>
#include <rerun/recording_stream.hpp>

#include <array>
#include <filesystem>
#include <vector>

namespace fs = std::filesystem;
namespace rr = rerun;
namespace rrc = rr::components;

#define TEST_TAG "[recording_stream]"

namespace rerun {
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
} // namespace rerun

SCENARIO("RecordingStream can be created, destroyed and lists correct properties", TEST_TAG) {
    const auto kind = GENERATE(rr::StoreKind::Recording, rr::StoreKind::Blueprint);

    GIVEN("recording stream kind" << kind) {
        AND_GIVEN("a valid application id") {
            THEN("creating a new stream does not log an error") {
                rr::RecordingStream stream =
                    check_logged_status([&] { return rr::RecordingStream("test", kind); });

                AND_THEN("it does not crash on destruction") {}

                AND_THEN("it reports the correct kind") {
                    CHECK(stream.kind() == kind);
                }
            }
        }

        AND_GIVEN("a nullptr for the application id") {
            THEN("creating a new stream logs a null argument error") {
                check_logged_status(
                    [&] { rr::RecordingStream stream(nullptr, kind); },
                    rr::ErrorCode::UnexpectedNullArgument
                );
            }
        }
        AND_GIVEN("invalid utf8 character sequence for the application id") {
            THEN("creating a new stream logs an invalid string argument error") {
                check_logged_status(
                    [&] { rr::RecordingStream stream("\xc3\x28", kind); },
                    rr::ErrorCode::InvalidStringArgument
                );
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
    for (auto kind : std::array{rr::StoreKind::Recording, rr::StoreKind::Blueprint}) {
        GIVEN("a store kind" << kind) {
            WHEN("creating a new stream") {
                rr::RecordingStream stream("test", kind);

                THEN("components as c-array can be logged") {
                    rrc::Point2D c_style_array[2] = {
                        rr::datatypes::Vec2D{1.0, 2.0},
                        rr::datatypes::Vec2D{4.0, 5.0},
                    };

                    stream.log_components("as-carray", c_style_array);
                }
                THEN("components as std::array can be logged") {
                    stream.log_components(
                        "as-array",
                        std::array<rrc::Point2D, 2>{
                            rr::datatypes::Vec2D{1.0, 2.0},
                            rr::datatypes::Vec2D{4.0, 5.0},
                        }
                    );
                }
                THEN("components as std::vector can be logged") {
                    stream.log_components(
                        "as-vector",
                        std::vector<rrc::Point2D>{
                            rr::datatypes::Vec2D{1.0, 2.0},
                            rr::datatypes::Vec2D{4.0, 5.0},
                        }
                    );
                }
                THEN("several components with a mix of vector, array and c-array can be logged") {
                    rrc::Label c_style_array[3] = {
                        rrc::Label("hello"),
                        rrc::Label("friend"),
                        rrc::Label("yo"),
                    };
                    stream.log_components(
                        "as-mix",
                        std::vector{
                            rrc::Point2D(rr::datatypes::Vec2D{0.0, 0.0}),
                            rrc::Point2D(rr::datatypes::Vec2D{1.0, 3.0}),
                            rrc::Point2D(rr::datatypes::Vec2D{5.0, 5.0}),
                        },
                        std::array{
                            rrc::Color(0xFF0000FF),
                            rrc::Color(0x00FF00FF),
                            rrc::Color(0x0000FFFF),
                        },
                        c_style_array
                    );
                }

                THEN("an archetype can be logged") {
                    stream.log_archetype(
                        "archetype",
                        rr::archetypes::Points2D({
                            rr::datatypes::Vec2D{1.0, 2.0},
                            rr::datatypes::Vec2D{4.0, 5.0},
                        })
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
        auto stream0 = std::make_unique<rr::RecordingStream>("test");

        AND_GIVEN("a nullptr for the save path") {
            THEN("then the save call returns a null argument error") {
                CHECK(stream0->save(nullptr).code == rr::ErrorCode::UnexpectedNullArgument);
            }
        }
        AND_GIVEN("valid save path " << test_rrd0) {
            AND_GIVEN("a directory already existing at this path") {
                fs::create_directory(test_rrd0.c_str());
                THEN("then the save call fails") {
                    CHECK(
                        stream0->save(test_rrd0.c_str()).code ==
                        rr::ErrorCode::RecordingStreamSaveFailure
                    );
                }
            }
            THEN("save call returns no error") {
                REQUIRE(stream0->save(test_rrd0.c_str()).is_ok());

                THEN("a new file got immediately created") {
                    CHECK(fs::exists(test_rrd0));
                }

                WHEN("creating a second stream") {
                    auto stream1 = std::make_unique<rr::RecordingStream>("test2");

                    WHEN("saving that one to a different file " << test_rrd1) {
                        REQUIRE(stream1->save(test_rrd1.c_str()).is_ok());

                        WHEN("logging a component to the second stream") {
                            check_logged_status([&] {
                                stream1->log_components(
                                    "as-array",
                                    std::array<rrc::Point2D, 2>{
                                        rr::datatypes::Vec2D{1.0, 2.0},
                                        rr::datatypes::Vec2D{4.0, 5.0},
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
                            check_logged_status([&] {
                                stream1->log_archetype(
                                    "archetype",
                                    rr::archetypes::Points2D({
                                        rr::datatypes::Vec2D{1.0, 2.0},
                                        rr::datatypes::Vec2D{4.0, 5.0},
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

void test_logging_to_connection(const char* address, rr::RecordingStream& stream) {
    AND_GIVEN("a nullptr for the socket address") {
        THEN("then the connect call returns a null argument error") {
            CHECK(stream.connect(nullptr, 0.0f).code == rr::ErrorCode::UnexpectedNullArgument);
        }
    }
    AND_GIVEN("an invalid address for the socket address") {
        THEN("then the save call fails") {
            CHECK(
                stream.connect("definitely not valid!", 0.0f).code ==
                rr::ErrorCode::InvalidSocketAddress
            );
        }
    }
    AND_GIVEN("a valid socket address " << address) {
        THEN("save call with zero timeout returns no error") {
            REQUIRE(stream.connect(address, 0.0f).is_ok());

            WHEN("logging a component and then flushing") {
                check_logged_status([&] {
                    stream.log_components(
                        "as-array",
                        std::array<rrc::Point2D, 2>{
                            rr::datatypes::Vec2D{1.0, 2.0},
                            rr::datatypes::Vec2D{4.0, 5.0},
                        }
                    );
                });
                stream.flush_blocking();

                THEN("does not crash") {
                    // No easy way to see if it got sent.
                }
            }
            WHEN("logging an archetype and then flushing") {
                check_logged_status([&] {
                    stream.log_archetype(
                        "archetype",
                        rr::archetypes::Points2D({
                            rr::datatypes::Vec2D{1.0, 2.0},
                            rr::datatypes::Vec2D{4.0, 5.0},
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
        rr::RecordingStream stream("test-local");
        test_logging_to_connection(address, stream);
    }
    WHEN("setting a global RecordingStream and then discarding it") {
        {
            rr::RecordingStream stream("test-global");
            stream.set_global();
        }
        GIVEN("the current recording stream") {
            test_logging_to_connection(address, rr::RecordingStream::current());
        }
    }
}

SCENARIO("Recording stream handles invalid logging gracefully", TEST_TAG) {
    GIVEN("a new RecordingStream") {
        rr::RecordingStream stream("test");

        AND_GIVEN("an invalid path") {
            auto variant = GENERATE(table<const char*, rr::ErrorCode>({
                std::tuple<const char*, rr::ErrorCode>("////", rr::ErrorCode::InvalidEntityPath),
                std::tuple<const char*, rr::ErrorCode>(
                    nullptr,
                    rr::ErrorCode::UnexpectedNullArgument
                ),
            }));
            const auto [path, error] = variant;
            auto v = rr::datatypes::Vec2D{1.0, 2.0};

            THEN("try_log_data_row returns the correct error") {
                CHECK(stream.try_log_data_row(path, 0, 0, nullptr).code == error);
            }
            THEN("try_log_components returns the correct error") {
                CHECK(
                    stream.try_log_components(path, std::array<rrc::Point2D, 1>{v}).code == error
                );
            }
            THEN("try_log_archetypes returns the correct error") {
                CHECK(stream.try_log_archetype(path, rr::archetypes::Points2D(v)).code == error);
            }
            THEN("log_components logs the correct error") {
                check_logged_status(
                    [&] {
                        stream.log_components(std::get<0>(variant), std::array<rrc::Point2D, 1>{v});
                    },
                    error
                );
            }
            THEN("log_archetypes logs the correct error") {
                check_logged_status(
                    [&] {
                        stream.log_archetype(std::get<0>(variant), rr::archetypes::Points2D(v));
                    },
                    error
                );
            }
        }

        AND_GIVEN("a valid path") {
            const char* path = "valid";
            AND_GIVEN("a cell with a null buffer") {
                rr::DataCell cell;
                cell.buffer = nullptr;
                cell.component_name = "valid";

                THEN("try_log_data_row fails with UnexpectedNullArgument") {
                    CHECK(
                        stream.try_log_data_row(path, 1, 1, &cell).code ==
                        rr::ErrorCode::UnexpectedNullArgument
                    );
                }
            }

            AND_GIVEN("a cell with a null component name") {
                rr::DataCell cell;
                cell.buffer = std::make_shared<arrow::Buffer>(nullptr, 0);
                cell.component_name = nullptr;

                THEN("try_log_data_row fails with UnexpectedNullArgument") {
                    CHECK(
                        stream.try_log_data_row(path, 1, 1, &cell).code ==
                        rr::ErrorCode::UnexpectedNullArgument
                    );
                }
            }

            AND_GIVEN("a cell with a valid component name but invalid data") {
                uint8_t invalid_data[1] = {0};
                rr::DataCell cell;
                cell.component_name = "very-valid";
                cell.buffer = std::make_shared<arrow::Buffer>(invalid_data, sizeof(invalid_data));

                THEN("try_log_data_row fails with ArrowIpcMessageParsingFailure") {
                    CHECK(
                        stream.try_log_data_row(path, 1, 1, &cell).code ==
                        rr::ErrorCode::ArrowIpcMessageParsingFailure
                    );
                }
            }

            // TODO(andreas): Missing test that provokes `ArrowDataCellError`. It's fairly hard to
            // get there which I reckon is a good thing!
        }
    }
}
