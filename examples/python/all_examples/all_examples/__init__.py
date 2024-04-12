from __future__ import annotations

import platform
from dataclasses import field, dataclass
from pathlib import Path
from typing import Iterable, Any

import tomli
from pyproject_metadata import StandardMetadata


@dataclass
class RerunMetadata:
    """Extract Rerun example metadata from a pyproject.toml data.

    Expected format in the pyproject.toml:

        [tool.rerun-example]
        skip = true
        extra-args = "--help"  # may also be a list
    """

    skip: bool
    extra_args: list[str]

    @classmethod
    def from_pyproject(cls, pyproject_data: dict[str, Any]) -> RerunMetadata:
        rerun_data = pyproject_data.get("tool", {}).get("rerun-example", {})

        skip = rerun_data.pop("skip", False)
        extra_args = rerun_data.pop("extra-args", [])
        if isinstance(extra_args, str):
            extra_args = [extra_args]

        if not len(rerun_data) == 0:
            raise ValueError(f"Unsupported fields in the rerun-example metadata: {', '.join(rerun_data.keys())}")

        return cls(skip=skip, extra_args=extra_args)


@dataclass
class Example:
    path: Path
    name: str = field(init=False)
    standard_metadata: StandardMetadata = field(init=False)
    rerun_metadata: RerunMetadata = field(init=False)

    def __post_init__(self):
        self.name = self.path.name
        pyproject_data = tomli.loads(Path(self.path / "pyproject.toml").read_text())
        self.standard_metadata = StandardMetadata.from_pyproject(pyproject_data, self.path)
        self.rerun_metadata = RerunMetadata.from_pyproject(pyproject_data)

    def active(self) -> bool:
        """Check that this example is active given its metadata and the current Python version."""
        if self.rerun_metadata.skip:
            return False

        requires_python = self.standard_metadata.requires_python
        if requires_python is not None:
            if not requires_python.contains(platform.python_version()):
                return False

        return True


def active_examples() -> Iterable[Example]:
    """Iterator over all active examples."""
    example_dir = Path(__file__).parent.parent.parent

    for example_path in example_dir.glob("*"):
        if example_path.is_dir() and (example_path / "pyproject.toml").exists() and example_path.name != "run_all":
            example = Example(example_path.absolute())

            if example.active():
                yield example
