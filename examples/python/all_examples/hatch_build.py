from __future__ import annotations

import sys
from pathlib import Path
from hatchling.metadata.plugin.interface import MetadataHookInterface

sys.path.append(str(Path(__file__).parent))

from all_examples import active_examples


# NOTE: useful command to debug what hatchling is doing:  python -u -W ignore -m hatchling metadata


class MetadataHook(MetadataHookInterface):
    def update(self, metadata: dict) -> None:
        """Use our very own package to list the examples we depend on."""

        # create a path-based dependency for all of our examples
        dependencies = [f"{example.name} @ file://{example.path}" for example in active_examples()]

        # other dependencies
        dependencies.extend(["pyproject-metadata", "tomli"])

        metadata["dependencies"] = dependencies
