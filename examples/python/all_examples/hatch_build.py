from __future__ import annotations

import sys
from pathlib import Path
from hatchling.metadata.plugin.interface import MetadataHookInterface

sys.path.append(str(Path(__file__).parent))

from all_examples import active_examples


class MetadataHook(MetadataHookInterface):
    def update(self, metadata: dict) -> None:
        """Use our very own package to list the examples we depend on.

        IMPORTANT: Do not print to stdout/stderr in his function, as it will end up being parsed. Use this command to
        check the output:

            python -m hatchling metadata
        """

        # create a path-based dependency for all of our examples
        dependencies = [
            f"{example.name} @ file://{example.path.absolute()} {example.environment_specifier()}"
            for example in active_examples()
        ]

        # other dependencies
        dependencies.extend(["pyproject-metadata", "tomli"])

        metadata["dependencies"] = dependencies
