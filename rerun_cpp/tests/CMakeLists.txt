cmake_minimum_required(VERSION 3.16...3.27)

# Catch2:
Include(FetchContent)
FetchContent_Declare(
    Catch2
    GIT_REPOSITORY https://github.com/catchorg/Catch2.git
    GIT_TAG v3.4.0
)

# To avoid that tests to not pass on Windows with BUILD_SHARED_LIBS=ON
# because the loader can't find Catch2.dll, we always compile Catch2 as static
if(WIN32 AND BUILD_SHARED_LIBS)
    set(BUILD_SHARED_LIBS_RERUN_SDK ${BUILD_SHARED_LIBS})
    set(BUILD_SHARED_LIBS OFF)
endif()

FetchContent_MakeAvailable(Catch2)

if(WIN32 AND BUILD_SHARED_LIBS_RERUN_SDK)
    set(BUILD_SHARED_LIBS ${BUILD_SHARED_LIBS_RERUN_SDK})
endif()

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
