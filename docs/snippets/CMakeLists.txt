cmake_minimum_required(VERSION 3.16...3.27)

# Setup builds for examples
file(GLOB_RECURSE sources_list CONFIGURE_DEPENDS ${CMAKE_CURRENT_SOURCE_DIR}/all/*.cpp)

# Not complete examples:
list(FILTER sources_list EXCLUDE REGEX .*/concepts/static/*)
list(FILTER sources_list EXCLUDE REGEX .*/migration/transactional_transforms/*)
list(FILTER sources_list EXCLUDE REGEX .*/tutorials/custom-application-id.*)
list(FILTER sources_list EXCLUDE REGEX .*/tutorials/custom-recording-id.*)
list(FILTER sources_list EXCLUDE REGEX .*/tutorials/log-file.*)
list(FILTER sources_list EXCLUDE REGEX .*/tutorials/log_line.*)
list(FILTER sources_list EXCLUDE REGEX .*/tutorials/quick_start.*)
list(FILTER sources_list EXCLUDE REGEX .*/tutorials/timelines_example.*)

add_custom_target(snippets)

foreach(SOURCE_PATH ${sources_list})
    get_filename_component(SOURCE_NAME ${SOURCE_PATH} NAME_WLE)

    if(${SOURCE_NAME} STREQUAL "CMakeFiles")
        CONTINUE()
    endif()

    set(EXAMPLE_TARGET ${SOURCE_NAME})

    # message(STATUS "Adding snippet: ${SOURCE_PATH}")
    add_executable(${EXAMPLE_TARGET} ${SOURCE_PATH})

    rerun_strict_warning_settings(${EXAMPLE_TARGET})
    target_link_libraries(${EXAMPLE_TARGET} PRIVATE rerun_sdk)

    add_dependencies(snippets ${EXAMPLE_TARGET})
endforeach()

# `text_log_integration` uses `loguru` as the example text logging library.
target_link_libraries(text_log_integration PRIVATE loguru::loguru)

# `any_values`-style examples use `arrow` directly.
target_link_libraries(any_batch_value_column_updates PRIVATE rerun_arrow_target)
target_link_libraries(any_values PRIVATE rerun_arrow_target)
target_link_libraries(any_values_column_updates PRIVATE rerun_arrow_target)
target_link_libraries(any_values_row_updates PRIVATE rerun_arrow_target)
target_link_libraries(dynamic_archetype PRIVATE rerun_arrow_target)
target_link_libraries(extra_values PRIVATE rerun_arrow_target)
target_link_libraries(recording_properties PRIVATE rerun_arrow_target)
