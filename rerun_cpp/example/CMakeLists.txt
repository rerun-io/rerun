cmake_minimum_required(VERSION 3.16)

project(RerunExample)

option(ARROW_LINK_SHARED "Link to the Arrow shared library" ON)

find_package(Arrow REQUIRED)

set(CMAKE_DL_LIBS "dl") # Required by Loguru for backtraces

# ------------------------------------------------------------------------------
# Loguru, see https://github.com/emilk/loguru/blob/master/loguru_cmake_example/CMakeLists.txt
include(FetchContent)
FetchContent_Declare(LoguruGitRepo
  GIT_REPOSITORY "https://github.com/emilk/loguru" # can be a filesystem path
  GIT_TAG "master"
)

# set any loguru compile-time flags before calling MakeAvailable()
set(LOGURU_STACKTRACES 1)
FetchContent_MakeAvailable(LoguruGitRepo) # defines target 'loguru::loguru'

# ------------------------------------------------------------------------------
if(NOT DEFINED CMAKE_CXX_STANDARD)
  set(CMAKE_CXX_STANDARD 17)
endif()

# Arrow requires a C++17 compliant compiler
set(CMAKE_CXX_STANDARD_REQUIRED ON)

if(NOT DEFINED CMAKE_BUILD_TYPE)
  set(CMAKE_BUILD_TYPE Release)
endif()

message(STATUS "Arrow version: ${ARROW_VERSION}")
message(STATUS "Arrow SO version: ${ARROW_FULL_SO_VERSION}")

add_executable(rerun_example main.cpp)

if(MSVC)
  target_compile_options(rerun_example PRIVATE /W4 /WX)
else()
  target_compile_options(rerun_example PRIVATE -Wall -Wextra -Wpedantic -Wcast-align -Wcast-qual -Wformat=2 -Wmissing-include-dirs -Wnull-dereference -Woverloaded-virtual -Wpointer-arith -Wshadow -Wswitch-enum -Wvla -Wno-sign-compare -Wconversion -Wunused -Wold-style-cast -Wno-missing-braces)
endif()

if(APPLE)
  target_link_libraries(rerun_example PRIVATE ${CMAKE_SOURCE_DIR}/../../target/debug/librerun_c.a)
  target_link_libraries(rerun_example PRIVATE "-framework CoreFoundation" "-framework IOKit")
endif()

if(LINUX)
  target_link_libraries(rerun_example PRIVATE ${CMAKE_SOURCE_DIR}/../../target/debug/librerun_c.a)
  target_link_libraries(rerun_example PRIVATE "-lm")
endif()

if(ARROW_LINK_SHARED)
  target_link_libraries(rerun_example PRIVATE Arrow::arrow_shared)
else()
  target_link_libraries(rerun_example PRIVATE Arrow::arrow_static)
endif()

include_directories(SYSTEM ${CMAKE_CURRENT_SOURCE_DIR}/../../crates/rerun_c/src) # for rerun.h (Rerun C SDK)
include_directories(SYSTEM ${CMAKE_CURRENT_SOURCE_DIR}/../src) # for rerun.hpp (Rerun C++ SDK)

target_link_libraries(rerun_example PRIVATE loguru::loguru)
