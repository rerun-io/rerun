import rerun as rr

rr.init("rerun_example_entity_path", spawn=True)

rr.log(r"world/42/escaped\ string\!", rr.TextDocument("This entity path was escaped manually"))
rr.log(
    ["world", 42, "unescaped string!"], rr.TextDocument("This entity path was provided as a list of unescaped strings")
)

assert rr.escape_entity_path_part("my string!") == r"my\ string\!"
assert rr.new_entity_path(["world", 42, "my string!"]) == r"/world/42/my\ string\!"
