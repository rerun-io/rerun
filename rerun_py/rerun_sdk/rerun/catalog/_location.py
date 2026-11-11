from __future__ import annotations

import warnings


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
