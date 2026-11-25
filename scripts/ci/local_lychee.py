"""Run Lychee with file URL remaps for the published docs redirects."""

from __future__ import annotations

import json
import re
import subprocess
import sys
from pathlib import Path
from tempfile import TemporaryDirectory

from .check_doc_redirects import DOCS_ROOT, load_redirects


def remap(source: str, destination: str) -> str:
    old = (DOCS_ROOT / f"{source}.md").as_uri()
    page, separator, fragment = destination.partition("#")
    if not page.startswith(("http://", "https://")):
        target = DOCS_ROOT / f"{page}.md"
        if not target.is_file() and (DOCS_ROOT / page).is_dir():
            target = DOCS_ROOT / page / "index.md"
        page = target.as_uri()

    # Keep the incoming fragment unless the redirect specifies one.
    replacement = (page + separator + fragment).replace("$", "$$")
    return rf"^{re.escape(old)}(#.*)?$ {replacement}{'' if separator else '$1'}"


def main() -> int:
    rules = [remap(source, dest) for source, dest in load_redirects().items()]
    with TemporaryDirectory() as temp:
        config = Path(temp) / "lychee.toml"
        config.write_text(f"remap = {json.dumps(rules)}\n" + Path("lychee.toml").read_text())
        return subprocess.run(["lychee", "--config", str(config), *sys.argv[1:]], check=False).returncode


if __name__ == "__main__":
    sys.exit(main())
