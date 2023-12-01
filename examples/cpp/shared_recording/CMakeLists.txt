cmake_minimum_required(VERSION 3.16...3.27)

# If you use the example outside of the Rerun SDK you need to specify
# where the rerun_c build is to be found by setting the `RERUN_CPP_URL` variable.
# This can be done by passing `-DRERUN_CPP_URL=<path to rerun_sdk_cpp zip>` to cmake.
if(DEFINED RERUN_REPOSITORY)
    add_executable(example_shared_recording main.cpp)
    rerun_strict_warning_settings(example_shared_recording)
else()
    project(example_shared_recording LANGUAGES CXX)

    add_executable(example_shared_recording main.cpp)

    # Set the path to the rerun_c build.
    set(RERUN_CPP_URL "https://github.com/rerun-io/rerun/releases/latest/download/rerun_cpp_sdk.zip" CACHE STRING "URL to the rerun_cpp zip.")
    option(RERUN_FIND_PACKAGE "Whether to use find_package to find a preinstalled rerun package (instead of using FetchContent)." OFF)

    if(RERUN_FIND_PACKAGE)
        find_package(rerun_sdk REQUIRED)
    else()
        # Download the rerun_sdk
        include(FetchContent)
        FetchContent_Declare(rerun_sdk URL ${RERUN_CPP_URL})
        FetchContent_MakeAvailable(rerun_sdk)
    endif()

    # Rerun requires at least C++17, but it should be compatible with newer versions.
    set_property(TARGET example_shared_recording PROPERTY CXX_STANDARD 17)
endif()

# Link against rerun_sdk.
target_link_libraries(example_shared_recording PRIVATE rerun_sdk)
