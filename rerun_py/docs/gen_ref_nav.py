"""Generate the code reference pages and navigation."""

from pathlib import Path

import mkdocs_gen_files

nav = mkdocs_gen_files.Nav()

nav["index"] = "index.md"

for path in sorted(Path("rerun").rglob("*.py")):
    module_path = path.relative_to(".").with_suffix("")
    full_doc_path = path.relative_to(".").with_suffix(".md")

    parts = tuple(module_path.parts)

    if parts[-1] == "__init__":
        parts = parts[:-1]
        full_doc_path = full_doc_path.with_name("index.md")
    elif parts[-1] == "__main__":
        continue

    nav[parts] = full_doc_path.as_posix()

    with mkdocs_gen_files.open(Path("package") / full_doc_path, "w") as fd:
        ident = ".".join(parts)
        fd.write(f"::: {ident}")

    # mkdocs_gen_files.set_edit_path(full_doc_path, Path("../") / path)

with mkdocs_gen_files.open("package/SUMMARY.txt", "w") as nav_file:
    nav_file.writelines(nav.build_literate_nav())
