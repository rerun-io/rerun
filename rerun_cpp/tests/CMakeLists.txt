cmake_minimum_required(VERSION 3.16)

# Catch2:
Include(FetchContent)
FetchContent_Declare(
    Catch2
    GIT_REPOSITORY https://github.com/catchorg/Catch2.git
    GIT_TAG v3.4.0
)
FetchContent_MakeAvailable(Catch2)

file(GLOB_RECURSE rerun_sdk_tests_SRC CONFIGURE_DEPENDS
    "*.hpp"
    "*.cpp"
)
add_executable(rerun_sdk_tests ${rerun_sdk_tests_SRC})

set_default_warning_settings(rerun_sdk_tests)

# Include arrow explicitely again, otherwise the arrow headers won't be declared as system headers.
# and instead become regular headers, causing warnings to be emitted.
target_link_libraries(rerun_sdk_tests PRIVATE loguru::loguru Catch2::Catch2 Arrow::arrow_static rerun_sdk)
