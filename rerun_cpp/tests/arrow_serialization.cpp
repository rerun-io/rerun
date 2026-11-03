#include <catch2/catch_test_macros.hpp>

#include <arrow/api.h>

#include <rerun/arrow_serialization.hpp>
#include <rerun/encodings/utf8.hpp>

using namespace rerun;

SCENARIO(
    "Arrow serialization propagates fill errors and skips empty fills", "[arrow_serialization]"
) {
    GIVEN("a fill callback that fails") {
        const int32_t value = 7;
        auto fail = [](arrow::ArrayBuilder*, const void*, size_t) {
            return Error(ErrorCode::InvalidComponent, "deliberate fill failure");
        };

        WHEN("a nonempty batch is serialized") {
            auto result = detail::serialize_arrow(arrow::int32(), &value, 1, fail);

            THEN("the fill error is returned unchanged") {
                CHECK(
                    result.error == Error(ErrorCode::InvalidComponent, "deliberate fill failure")
                );
            }
        }

        WHEN("the instance pointer is null") {
            auto result = detail::serialize_arrow(arrow::int32(), nullptr, 1, fail);

            THEN("an empty array is returned without calling fill") {
                REQUIRE(result.is_ok());
                CHECK(result.value->length() == 0);
                CHECK(result.value->type()->Equals(arrow::int32()));
            }
        }

        WHEN("the instance count is zero") {
            auto result = detail::serialize_arrow(arrow::int32(), &value, 0, fail);

            THEN("an empty array is returned without calling fill") {
                REQUIRE(result.is_ok());
                CHECK(result.value->length() == 0);
                CHECK(result.value->type()->Equals(arrow::int32()));
            }
        }
    }
}

SCENARIO("Arrow serialization finishes filled arrays", "[arrow_serialization]") {
    GIVEN("an integer and a matching typed fill callback") {
        const int32_t value = 7;

        WHEN("the integer is serialized") {
            auto result = detail::serialize_arrow(
                arrow::int32(),
                &value,
                1,
                [](arrow::ArrayBuilder* builder, const void* instances, size_t count) -> Error {
                    CHECK(count == 1);
                    return static_cast<arrow::Int32Builder*>(builder)->Append(
                        *static_cast<const int32_t*>(instances)
                    );
                }
            );

            THEN("the finished array contains the input value") {
                REQUIRE(result.is_ok());
                REQUIRE(result.value->length() == 1);
                CHECK(result.value->ValidateFull().ok());
                CHECK(std::static_pointer_cast<arrow::Int32Array>(result.value)->Value(0) == value);
            }
        }
    }

    GIVEN("binary data containing an embedded null") {
        const std::string bytes("a\0b", 3);

        WHEN("the data is serialized with a matching typed fill callback") {
            auto result = detail::serialize_arrow(
                arrow::binary(),
                &bytes,
                1,
                [](arrow::ArrayBuilder* builder, const void* instances, size_t) -> Error {
                    return static_cast<arrow::BinaryBuilder*>(builder)->Append(
                        *static_cast<const std::string*>(instances)
                    );
                }
            );

            THEN("the finished array retains every byte") {
                REQUIRE(result.is_ok());
                arrow::BinaryBuilder expected;
                REQUIRE(expected.Append(bytes).ok());
                CHECK(result.value->Equals(expected.Finish().ValueOrDie()));
            }
        }
    }
}

SCENARIO("Typed serialization adapters use the shared lifecycle", "[arrow_serialization]") {
    GIVEN("a string encoding and its matching Arrow builder type") {
        const encodings::Utf8 value("hello");

        WHEN("the typed adapter serializes the value") {
            auto result = detail::serialize_arrow<arrow::StringBuilder>(&value, 1);

            THEN("the finished array contains the string") {
                REQUIRE(result.is_ok());
                arrow::StringBuilder expected;
                REQUIRE(expected.Append("hello").ok());
                CHECK(result.value->Equals(expected.Finish().ValueOrDie()));
            }
        }

        WHEN("the typed instance pointer is null with a positive count") {
            auto result = detail::serialize_arrow<arrow::StringBuilder>(
                static_cast<const encodings::Utf8*>(nullptr),
                1
            );

            THEN("the shared lifecycle returns an empty array without filling") {
                REQUIRE(result.is_ok());
                CHECK(result.value->length() == 0);
                CHECK(result.value->type()->Equals(arrow::utf8()));
            }
        }

        WHEN("the instance count is zero") {
            auto result = detail::serialize_arrow<arrow::StringBuilder>(&value, 0);

            THEN("the shared lifecycle returns an empty array without filling") {
                REQUIRE(result.is_ok());
                CHECK(result.value->length() == 0);
                CHECK(result.value->type()->Equals(arrow::utf8()));
            }
        }
    }
}

SCENARIO("Arrow builder argument checks preserve their diagnostics", "[arrow_serialization]") {
    GIVEN("a builder and an input value") {
        arrow::Int32Builder builder;
        const int32_t value = 7;

        WHEN("the builder is null") {
            auto error = detail::check_arrow_builder_arguments(nullptr, &value);

            THEN("the error identifies the missing builder") {
                CHECK(
                    error ==
                    Error(ErrorCode::UnexpectedNullArgument, "Passed array builder is null.")
                );
            }
        }

        WHEN("the input is null") {
            auto error = detail::check_arrow_builder_arguments(&builder, nullptr);

            THEN("the error identifies the missing input") {
                CHECK(
                    error == Error(
                                 ErrorCode::UnexpectedNullArgument,
                                 "Cannot serialize null pointer to arrow array."
                             )
                );
            }
        }

        WHEN("both arguments are null") {
            auto error = detail::check_arrow_builder_arguments(nullptr, nullptr);

            THEN("the builder error takes precedence") {
                CHECK(
                    error ==
                    Error(ErrorCode::UnexpectedNullArgument, "Passed array builder is null.")
                );
            }
        }

        WHEN("both arguments are non-null") {
            THEN("the arguments are accepted") {
                CHECK(detail::check_arrow_builder_arguments(&builder, &value).is_ok());
            }
        }
    }
}
