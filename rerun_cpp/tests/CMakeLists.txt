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
add_executable(rerun_sdk_tests ${rerun_sdk_tests_SRC})

set_default_warning_settings(rerun_sdk_tests)

# -----------------------------------------------------------------------------
# Arrow:

# TODO: copy pasted from parent CMakeLists.txt
target_include_directories(rerun_sdk_tests SYSTEM PRIVATE ${ARROW_DOWNLOAD_PATH}/include)

# message(STATUS "Arrow version: ${ARROW_VERSION}")
# message(STATUS "Arrow SO version: ${ARROW_FULL_SO_VERSION}")
#
# if(ARROW_LINK_SHARED)
# target_link_libraries(rerun_sdk_tests PRIVATE Arrow::arrow_shared)
# else()
# target_link_libraries(rerun_sdk_tests PRIVATE Arrow::arrow_static)
# endif()

# Include arrow explicitly again, otherwise the arrow headers won't be declared as system headers.
# and instead become regular headers, causing warnings to be emitted.
target_link_libraries(rerun_sdk_tests PRIVATE loguru::loguru Catch2::Catch2 rerun_sdk)
