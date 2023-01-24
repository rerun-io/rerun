"""Generate the code reference pages and navigation."""

from pathlib import Path

import mkdocs_gen_files

nav = mkdocs_gen_files.Nav()

nav["index"] = "index.md"

for path in sorted(Path("rerun").rglob("*.py")):
    rel_path = path.relative_to(".")
    module_path = rel_path.with_suffix("")
    doc_path = rel_path.with_suffix(".md")
    write_path = Path("package") / doc_path

    nav_parts = tuple(p + "/" for p in path.parts[:-1]) + (path.parts[-1],)
    ident_parts = tuple(module_path.parts)

    if ident_parts[-1] == "__init__":
        ident_parts = ident_parts[:-1]
    elif ident_parts[-1] == "__main__":
        continue

    nav[nav_parts] = doc_path.as_posix()

    with mkdocs_gen_files.open(write_path, "w") as fd:
        ident = ".".join(ident_parts)
        fd.write(f"::: {ident}")

    # mkdocs_gen_files.set_edit_path(full_doc_path, Path("../") / path)

with mkdocs_gen_files.open("package/SUMMARY.txt", "w") as nav_file:
    nav_file.writelines(nav.build_literate_nav())
