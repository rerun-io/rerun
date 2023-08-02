cmake_minimum_required(VERSION 3.19)

# Catch2:
Include(FetchContent)
FetchContent_Declare(
    Catch2
    GIT_REPOSITORY https://github.com/catchorg/Catch2.git
    GIT_TAG v3.4.0
)
FetchContent_MakeAvailable(Catch2)

# Test application:
if(NOT DEFINED CMAKE_CXX_STANDARD)
    set(CMAKE_CXX_STANDARD 17)
endif()

file(GLOB_RECURSE rerun_sdk_tests_SRC CONFIGURE_DEPENDS
    "*.hpp"
    "*.cpp"
)
add_executable(rerun_sdk_tests ${rerun_sdk_tests_SRC})

set_default_warning_settings(rerun_sdk_tests)

target_link_libraries(rerun_sdk_tests PRIVATE Catch2::Catch2WithMain rerun_sdk)
