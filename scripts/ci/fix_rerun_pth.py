#!/usr/bin/env python3
"""Fix up the rerun_sdk.pth file after maturin develop.

maturin develop installs an editable .pth file that points to rerun_py/,
but we need it to also include rerun_py/rerun_sdk/ so that `import rerun` works.

The .pth file needs both paths:
- rerun_py/ for rerun_bindings (the compiled extension module)
- rerun_py/rerun_sdk/ for the rerun package itself
"""

from __future__ import annotations

import sysconfig
from pathlib import Path


def main() -> None:
    site_packages = Path(sysconfig.get_paths()["purelib"])
    pth_file = site_packages / "rerun_sdk.pth"

    if not pth_file.exists():
        print(f"Warning: {pth_file} does not exist, skipping fixup")
        return

    current_content = pth_file.read_text().strip()
    lines = current_content.splitlines()

    # Check if it's maturin's single-line editable install pointing to rerun_py
    if len(lines) == 1 and lines[0].endswith("rerun_py"):
        rerun_py_path = lines[0]
        new_content = f"{rerun_py_path}\n{rerun_py_path}/rerun_sdk\n"
        pth_file.write_text(new_content)
        print(f"Fixed {pth_file}:")
        print(f"  - {rerun_py_path} (for rerun_bindings)")
        print(f"  - {rerun_py_path}/rerun_sdk (for rerun package)")
    elif len(lines) == 2 and lines[0].endswith("rerun_py") and lines[1].endswith("rerun_py/rerun_sdk"):
        print(f"Already fixed: {pth_file}")
    else:
        print(f"Unexpected content in {pth_file}:")
        for line in lines:
            print(f"  {line}")


if __name__ == "__main__":
    main()
