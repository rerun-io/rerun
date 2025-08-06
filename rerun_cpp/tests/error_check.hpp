#pragma once

#include <catch2/catch_test_macros.hpp>

#include <rerun/error.hpp>

/// Checks if the given operation logs the expected status code.
template <typename Op>
auto check_logged_error(
    Op operation, rerun::ErrorCode expected_status_code = rerun::ErrorCode::Ok
) {
    static rerun::Error last_logged_status;

    // Set to Ok since nothing logged indicates success for most methods.
    last_logged_status.code = rerun::ErrorCode::Ok;

    rerun::Error::set_log_handler(
        [](const rerun::Error& status, void* userdata) {
            *static_cast<rerun::Error*>(userdata) = status;
        },
        &last_logged_status
    );

    struct CheckOnDestruct {
        rerun::ErrorCode expected_status_code;

        ~CheckOnDestruct() {
            CHECK(last_logged_status.code == expected_status_code);
            if (expected_status_code != rerun::ErrorCode::Ok) {
                CHECK(last_logged_status.description.length() > 0);
            } else {
                CHECK(last_logged_status.description == "");
            }
            rerun::Error::set_log_handler(nullptr);
        }
    } check = {expected_status_code};

    // `auto result = operation();` won't compile for void
    // but `return operation();` is just fine.
    return operation();
}
