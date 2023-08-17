#include <catch2/catch_test_macros.hpp>

#include <rerun/status.hpp>

/// Checks if the given operation logs the expected status code.
template <typename Op>
auto check_logged_status(
    Op operation, rerun::StatusCode expected_status_code = rerun::StatusCode::Ok
) {
    static rerun::Status last_logged_status;

    // Set to Ok since nothing logged indicates success for most methods.
    last_logged_status.code = rerun::StatusCode::Ok;

    rerun::Status::set_log_handler(
        [](const rerun::Status& status, void* userdata) {
            *static_cast<rerun::Status*>(userdata) = status;
        },
        &last_logged_status
    );

    struct CheckOnDestruct {
        rerun::StatusCode expected_status_code;

        ~CheckOnDestruct() {
            CHECK(last_logged_status.code == expected_status_code);
            if (expected_status_code != rerun::StatusCode::Ok) {
                CHECK(last_logged_status.description.length() > 0);
            } else {
                CHECK(last_logged_status.description.length() == 0);
            }
            rerun::Status::set_log_handler(nullptr);
        }
    } check = {expected_status_code};

    // `auto result = operation();` won't compile for void
    // but `return operation();` is just fine.
    return operation();
}
