cmake_minimum_required(VERSION 3.16...3.27)

# Instead of building separate executables (one per snippet), we build a single
# "snippets" binary with a dispatcher. Each snippet's `main` is renamed to `<name>_main`
# at configure time, and a generated dispatcher routes `argv[1]` to the right function.
file(GLOB_RECURSE sources_list CONFIGURE_DEPENDS ${CMAKE_CURRENT_SOURCE_DIR}/all/*.cpp)

# Not complete examples (code fragments without main):
list(FILTER sources_list EXCLUDE REGEX .*/concepts/static/*)
list(FILTER sources_list EXCLUDE REGEX .*/migration/transactional_transforms/*)
list(FILTER sources_list EXCLUDE REGEX .*/tutorials/custom-application-id.*)
list(FILTER sources_list EXCLUDE REGEX .*/tutorials/custom-recording-id.*)
list(FILTER sources_list EXCLUDE REGEX .*/tutorials/log-file.*)
list(FILTER sources_list EXCLUDE REGEX .*/tutorials/log_line.*)
list(FILTER sources_list EXCLUDE REGEX .*/tutorials/quick_start.*)
list(FILTER sources_list EXCLUDE REGEX .*/tutorials/timelines_example.*)

set(GENERATED_DIR "${CMAKE_CURRENT_BINARY_DIR}/generated_snippets")
file(MAKE_DIRECTORY "${GENERATED_DIR}")

set(ALL_GENERATED_SOURCES "")
set(FORWARD_DECLARATIONS "")
set(DISPATCH_ENTRIES "")
set(SNIPPET_NAMES "")

foreach(SOURCE_PATH ${sources_list})
    get_filename_component(SOURCE_NAME ${SOURCE_PATH} NAME_WLE)

    if(${SOURCE_NAME} STREQUAL "CMakeFiles")
        continue()
    endif()

    # C++ identifier: replace dashes with underscores.
    string(REPLACE "-" "_" FUNC_NAME "${SOURCE_NAME}")

    # Read the original source and validate the entry point signature.
    file(READ "${SOURCE_PATH}" SOURCE_CONTENT)
    string(FIND "${SOURCE_CONTENT}" "int main(int argc, char* argv[])" _pos)

    if(_pos EQUAL -1)
        message(FATAL_ERROR
            "Snippet '${SOURCE_PATH}' does not use the expected signature: "
            "int main(int argc, char* argv[]). "
            "All snippets must use this exact signature."
        )
    endif()

    # Rename main -> <func_name>_main.
    string(REPLACE
        "int main(int argc, char* argv[])"
        "int ${FUNC_NAME}_main(int argc, char* argv[])"
        SOURCE_CONTENT "${SOURCE_CONTENT}"
    )

    # Make non-main helper functions `static` to avoid duplicate symbols when
    # multiple snippets define functions with the same name (e.g. `run_main`, `truncated_radians`).
    # If a new snippet adds a helper function that conflicts, the linker will error with
    # "duplicate symbol" - add a new REPLACE line here to fix it.
    string(REPLACE "arrow::Status run_main()" "static arrow::Status run_main()" SOURCE_CONTENT "${SOURCE_CONTENT}")
    string(REPLACE "float truncated_radians(int deg)" "static float truncated_radians(int deg)" SOURCE_CONTENT "${SOURCE_CONTENT}")

    # Prepend per-file fixups before the original source:
    # - #define renames for user types that would otherwise cause ODR violations across snippets
    # (macros don't expand in string literals, so component name strings are preserved).
    # Add new defines here if a new snippet introduces a conflicting type name.
    # - #line restores __FILE__ to the original path so that snippets using
    # `fs::path(__FILE__).parent_path()` (e.g. encoded_image.cpp) find adjacent assets.
    string(PREPEND SOURCE_CONTENT
        "#define CustomPosition3D ${FUNC_NAME}_CustomPosition3D\n"
        "#define CustomPoints3D ${FUNC_NAME}_CustomPoints3D\n"
        "#define Confidence ${FUNC_NAME}_Confidence\n"
        "#line 1 \"${SOURCE_PATH}\"\n"
    )

    # If the last statement before the closing `}` isn't a `return`, add `return 0;`.
    # This handles cases where `return` only appears in nested blocks (e.g. `if (argc < 2) return 1;`)
    # but the main function body falls off the end without returning.
    string(REGEX MATCH "return [^;]*;[ \t\r\n]*}[ \t\r\n]*$" _ends_with_return "${SOURCE_CONTENT}")

    if(NOT _ends_with_return)
        string(REGEX REPLACE "}[ \t\r\n]*$" "    return 0;\n}\n" SOURCE_CONTENT "${SOURCE_CONTENT}")
    endif()

    # Write the transformed source.
    file(WRITE "${GENERATED_DIR}/${FUNC_NAME}.cpp" "${SOURCE_CONTENT}")
    list(APPEND ALL_GENERATED_SOURCES "${GENERATED_DIR}/${FUNC_NAME}.cpp")

    # Build dispatcher data.
    # CLI name keeps original dashes.
    string(APPEND FORWARD_DECLARATIONS "int ${FUNC_NAME}_main(int argc, char* argv[]);\n")
    string(APPEND DISPATCH_ENTRIES "    {\"${SOURCE_NAME}\", ${FUNC_NAME}_main},\n")
    list(APPEND SNIPPET_NAMES "${SOURCE_NAME}")
endforeach()

# Sort snippet names for the usage message.
list(SORT SNIPPET_NAMES)
set(SNIPPET_LIST_LITERAL "")

foreach(_name ${SNIPPET_NAMES})
    string(APPEND SNIPPET_LIST_LITERAL "        \"${_name}\",\n")
endforeach()

# Generate the dispatcher from template, substituting @VARIABLES@.
configure_file(
    "${CMAKE_CURRENT_SOURCE_DIR}/snippet_dispatcher.cpp.in"
    "${GENERATED_DIR}/snippet_dispatcher.cpp"
    @ONLY
)

# Build the single executable.
add_executable(snippets
    "${GENERATED_DIR}/snippet_dispatcher.cpp"
    ${ALL_GENERATED_SOURCES}
)

rerun_strict_warning_settings(snippets)

# Renamed mains lose the compiler's special `main` treatment that suppresses
# unused-parameter warnings for argc/argv.
if(MSVC)
    target_compile_options(snippets PRIVATE /wd4100)
else()
    target_compile_options(snippets PRIVATE -Wno-unused-parameter)
endif()

target_link_libraries(snippets PRIVATE rerun_sdk loguru::loguru rerun_arrow_target)
