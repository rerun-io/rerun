cmake_minimum_required(VERSION 3.16)

# project(RerunExample)

# ------------------------------------------------------------------------------

# Arrow requires a C++17 compliant compiler
if(NOT DEFINED CMAKE_CXX_STANDARD)
  set(CMAKE_CXX_STANDARD 17)
endif()

if(NOT DEFINED CMAKE_BUILD_TYPE)
  set(CMAKE_BUILD_TYPE Release)
endif()

add_executable(rerun_example main.cpp)

# TODO(andreas): Fix warnings and enable this.
# set_default_warning_settings(rerun_example)
target_link_libraries(rerun_example PRIVATE loguru::loguru rerun_sdk ${CMAKE_CURRENT_SOURCE_DIR}/../../../target/release/librerun_c.a)
