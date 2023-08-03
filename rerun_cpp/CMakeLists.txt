cmake_minimum_required(VERSION 3.16)

add_subdirectory(tests)

# NOTE: CMake docs strongly discourages using GLOB, and instead suggests
# manually listing all the files, like it's 1972.
# However, that won't work for use since we auto-generate the source tree.
# See https://cmake.org/cmake/help/latest/command/file.html#glob
file(GLOB_RECURSE rerun_sdk_SRC CONFIGURE_DEPENDS
    "src/*.hpp"
    "src/*.cpp"
)

add_library(rerun_sdk ${rerun_sdk_SRC})

# TODO(andreas): Fix all warnings and enable this.
# set_default_warning_settings(rerun_sdk)

# ------------------------------------------------------------------------------

# For rerun.h (Rerun C SDK):
include_directories(SYSTEM ${CMAKE_CURRENT_SOURCE_DIR}/../crates/rerun_c/src)

# Make sure the compiler can find include files for rerun
# when other libraries or executables link to rerun:
# TODO(andreas): These should be prefixed `rerun/`.
target_include_directories(rerun_sdk PUBLIC ${CMAKE_CURRENT_SOURCE_DIR}/src)

# ------------------------------------------------------------------------------
# Loguru logging library (https://github.com/emilk/loguru):
set(CMAKE_DL_LIBS "dl") # Required by Loguru for backtraces

# Loguru, see https://github.com/emilk/loguru/blob/master/loguru_cmake_example/CMakeLists.txt
include(FetchContent)
FetchContent_Declare(LoguruGitRepo
    GIT_REPOSITORY "https://github.com/emilk/loguru" # can be a filesystem path
    GIT_TAG "master"
)

# set any loguru compile-time flags before calling MakeAvailable()
set(LOGURU_STACKTRACES 1)
FetchContent_MakeAvailable(LoguruGitRepo) # defines target 'loguru::loguru'

target_link_libraries(rerun_sdk PRIVATE loguru::loguru)

# ------------------------------------------------------------------------------
# execute process doesn't fail if the process fails.
# `COMMAND_ERROR_IS_FATAL ANY` parameter fixes this but is only available in CMake 3.19
# which isn't default on Ubuntu LTS as of writing, causing unnecessary friction.
execute_process(COMMAND cargo build --release -p re_types RESULT_VARIABLE ret) # Generates most of the C++ source files

if(NOT(ret EQUAL "0"))
    message(FATAL_ERROR "Failed to build re_types in order to generate C++ source files.")
endif()

execute_process(COMMAND cargo build --release -p rerun_c RESULT_VARIABLE ret) # We link against this, so must be up-to-date

if(NOT(ret EQUAL "0"))
    message(FATAL_ERROR "Failed to build rerun_c.")
endif()

# ------------------------------------------------------------------------------
if(APPLE)
    target_link_libraries(rerun_sdk PRIVATE ${CMAKE_CURRENT_SOURCE_DIR}/../target/release/librerun_c.a)
    target_link_libraries(rerun_sdk PRIVATE "-framework CoreFoundation" "-framework IOKit")
elseif(UNIX) # if(LINUX) # CMake 3.25
    target_link_libraries(rerun_sdk PRIVATE ${CMAKE_CURRENT_SOURCE_DIR}/../target/release/librerun_c.a)
    target_link_libraries(rerun_sdk PRIVATE "-lm")
elseif(WIN32)
    target_link_libraries(rerun_sdk PRIVATE ${CMAKE_CURRENT_SOURCE_DIR}/../target/release/librerun_c.a)

    # TODO(andreas): Untested. Almost certainly gonna encounter issues here.
endif()

# -----------------------------------------------------------------------------
# Arrow:
option(ARROW_LINK_SHARED "Link to the Arrow shared library" ON)

find_package(Arrow REQUIRED)

# Arrow requires a C++17 compliant compiler
set(CMAKE_CXX_STANDARD_REQUIRED ON)

message(STATUS "Arrow version: ${ARROW_VERSION}")
message(STATUS "Arrow SO version: ${ARROW_FULL_SO_VERSION}")

if(ARROW_LINK_SHARED)
    target_link_libraries(rerun_sdk PRIVATE Arrow::arrow_shared)
else()
    target_link_libraries(rerun_sdk PRIVATE Arrow::arrow_static)
endif()
