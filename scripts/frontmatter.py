from __future__ import annotations

from typing import Any, Dict

import tomlkit

Frontmatter = Dict[str, Any]


def load_frontmatter(s: str) -> dict[str, Any] | None:
    start = s.find("<!--[metadata]")
    if start == -1:
        return None
    start += len("<!--[metadata]")

    end = s.find("-->", start)
    if end == -1:
        return None

    fm = s[start:end].strip()

    return tomlkit.loads(fm).unwrap()
