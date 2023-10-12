add_subdirectory(clock)
add_subdirectory(minimal)

add_custom_target(examples)
add_dependencies(examples example_minimal)
add_dependencies(examples example_clock)
