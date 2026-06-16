#!/usr/bin/env python3
"""Regenerate static blueprint resources for E2E redap tests."""

from __future__ import annotations

from pathlib import Path

import rerun.blueprint as rrb

BLUEPRINTS = {
    "table_blueprint.rbl": [-1, 2],
    "table_blueprint2.rbl": [-2, 3],
}


def main() -> None:
    base = Path(__file__).parent

    for filename, x_range in BLUEPRINTS.items():
        blueprint = rrb.Blueprint(rrb.Spatial2DView(visual_bounds=rrb.VisualBounds2D(x_range=x_range, y_range=[-1, 2])))
        blueprint.save(f"e2e_{filename}", base / filename)


if __name__ == "__main__":
    main()
