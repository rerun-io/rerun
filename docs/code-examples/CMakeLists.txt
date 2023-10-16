cmake_minimum_required(VERSION 3.16)

# Setup builds for examples
file(GLOB sources_list true ${CMAKE_CURRENT_SOURCE_DIR}/*.cpp)

add_custom_target(doc_examples)

foreach(SOURCE_PATH ${sources_list})
    get_filename_component(SOURCE_NAME ${SOURCE_PATH} NAME_WLE)

    if(${SOURCE_NAME} STREQUAL "CMakeFiles")
        CONTINUE()
    endif()

    set(EXAMPLE_TARGET ${SOURCE_NAME})

    add_executable(${EXAMPLE_TARGET} ${SOURCE_PATH})

    set_default_warning_settings(${EXAMPLE_TARGET})
    target_link_libraries(${EXAMPLE_TARGET} PRIVATE rerun_sdk)

    add_dependencies(doc_examples ${EXAMPLE_TARGET})
endforeach()
