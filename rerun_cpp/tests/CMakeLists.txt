cmake_minimum_required(VERSION 3.16...3.27)

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

# Add readme snippets to the tests to make sure they keep compiling.
list(APPEND rerun_sdk_tests_SRC "../docs/readme_snippets.cpp")

add_executable(rerun_sdk_tests ${rerun_sdk_tests_SRC})

rerun_strict_warning_settings(rerun_sdk_tests)

# Include arrow explicitly again, otherwise the arrow headers won't be found.
target_link_libraries(rerun_sdk_tests PRIVATE loguru::loguru Catch2::Catch2 rerun_sdk rerun_arrow_target)
