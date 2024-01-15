cmake_minimum_required(VERSION 3.16...3.27)

# Setup builds for examples
file(GLOB sources_list true ${CMAKE_CURRENT_SOURCE_DIR}/all/*.cpp)

# TODO(ab): incomplete code, need hideable stubs, see https://github.com/rerun-io/landing/issues/515
list(REMOVE_ITEM sources_list ${CMAKE_CURRENT_SOURCE_DIR}/all/timelines_example.cpp)

add_custom_target(doc_examples)

foreach(SOURCE_PATH ${sources_list})
    get_filename_component(SOURCE_NAME ${SOURCE_PATH} NAME_WLE)

    if(${SOURCE_NAME} STREQUAL "CMakeFiles")
        CONTINUE()
    endif()

    set(EXAMPLE_TARGET ${SOURCE_NAME})

    add_executable(${EXAMPLE_TARGET} ${SOURCE_PATH})

    rerun_strict_warning_settings(${EXAMPLE_TARGET})
    target_link_libraries(${EXAMPLE_TARGET} PRIVATE rerun_sdk)

    add_dependencies(doc_examples ${EXAMPLE_TARGET})
endforeach()

# `text_log_integration` uses `loguru` as the example text logging library.
target_link_libraries(text_log_integration PRIVATE rerun_sdk loguru::loguru)
