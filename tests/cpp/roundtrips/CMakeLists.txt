cmake_minimum_required(VERSION 3.16...3.27)

file(GLOB sources_list LIST_DIRECTORIES true ${CMAKE_CURRENT_SOURCE_DIR}/*)

foreach(DIR ${sources_list})
    IF(IS_DIRECTORY ${DIR})
        get_filename_component(ARCHETYPE ${DIR} NAME)

        if(${ARCHETYPE} STREQUAL "CMakeFiles")
            CONTINUE()
        endif()

        set(ROUNDTRIP_TARGET roundtrip_${ARCHETYPE})

        add_executable(${ROUNDTRIP_TARGET} ${DIR}/main.cpp)
        rerun_strict_warning_settings(${ROUNDTRIP_TARGET})
        target_link_libraries(${ROUNDTRIP_TARGET} PRIVATE rerun_sdk)
    ELSE()
        CONTINUE()
    ENDIF()
endforeach()
