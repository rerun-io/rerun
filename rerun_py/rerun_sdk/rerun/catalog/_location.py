from __future__ import annotations

import warnings
from dataclasses import dataclass


@dataclass(frozen=True)
class StorageLocation:
    """A storage location configured in this catalog deployment."""

    alias: str
    """Deployment-local alias accepted by `storage_location` during registration."""

    display_name: str | None
    region: str
    bucket_name: str
    prefix: str | None
    """Object keys are relative to this prefix."""

    read_only: bool
    """The location forbids writes even if its credentials permit them."""


@dataclass(frozen=True)
class StorageLocations:
    """Storage locations and the alias used when none is specified."""

    locations: list[StorageLocation]
    default_alias: str

    def __repr__(self) -> str:
        if not self.locations:
            return "StorageLocations(empty)"

        rows = [("Alias", "Name", "Region", "Bucket", "Prefix", "Access")]
        rows.extend(
            (
                location.alias + (" (default)" if location.alias == self.default_alias else ""),
                location.display_name or "",
                location.region,
                location.bucket_name,
                location.prefix or "",
                "read-only" if location.read_only else "read-write",
            )
            for location in sorted(self.locations, key=lambda location: location.alias)
        )
        widths = [max(len(cell) for cell in column) for column in zip(*rows, strict=True)]
        lines = ["StorageLocations"]
        for index, row in enumerate(rows):
            lines.append("  " + "  ".join(cell.ljust(width) for cell, width in zip(row, widths, strict=True)).rstrip())
            if index == 0:
                lines.append("  " + "  ".join("-" * width for width in widths))
        return "\n".join(lines)


# TODO(RR-6315): Remove this resolver when object_store_config support ends.
def _resolve_storage_location(
    storage_location: str | None, object_store_config: str | None, *, stacklevel: int = 3
) -> str | None:
    if storage_location is not None and object_store_config is not None:
        raise ValueError("storage_location and object_store_config cannot both be supplied")
    if object_store_config is not None:
        warnings.warn(
            "object_store_config is deprecated; use storage_location instead.",
            DeprecationWarning,
            stacklevel=stacklevel,
        )
    return storage_location if storage_location is not None else object_store_config
