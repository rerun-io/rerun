cmake_minimum_required(VERSION 3.16)

# If you use the example outside of the Rerun SDK you need to specify
# where the rerun_c build is to be found by setting the `RERUN_CPP_URL` variable.
# This can be done by passing `-DRERUN_CPP_URL=<path to rerun_sdk_cpp zip>` to cmake.
if(DEFINED RERUN_REPOSITORY)
    add_executable(example_spawn_viewer main.cpp)
    set_default_warning_settings(example_spawn_viewer)
else()
    project(example_spawn_viewer LANGUAGES CXX)

    add_executable(example_spawn_viewer main.cpp)

    # Set the path to the rerun_c build.
    set(RERUN_CPP_URL CACHE STRING "URL to the rerun_cpp zip.")

    # Download the rerun_sdk
    include(FetchContent)
    FetchContent_Declare(rerun_sdk DOWNLOAD_EXTRACT_TIMESTAMP YES URL ${RERUN_CPP_URL})
    FetchContent_MakeAvailable(rerun_sdk)

    # Rerun requires at least C++17, but it should be compatible with newer versions.
    set_property(TARGET example_spawn_viewer PROPERTY CXX_STANDARD 17)
endif()

# Link against rerun_sdk.
target_link_libraries(example_spawn_viewer PRIVATE rerun_sdk)
