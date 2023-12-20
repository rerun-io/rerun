import rerun as rr

rr.init("rerun_example_entity_path", spawn=True)

rr.log(r"world/escaped\ string\!", rr.TextDocument("This entity path was escaped manually"))
rr.log(["world", "unescaped string!"], rr.TextDocument("This entity path was provided as a list of unescaped strings"))
