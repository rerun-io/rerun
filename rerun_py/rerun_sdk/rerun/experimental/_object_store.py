"""
Authentication for direct object store fetches made by dataframe queries.

Pass an instance of one of these classes to [`CatalogClient`][rerun.catalog.CatalogClient].
Without one, the server is asked for pre-signed URLs.
"""

from __future__ import annotations

from rerun_bindings import BearerTokenObjectStoreAuth as BearerTokenObjectStoreAuth

ObjectStoreAuth = BearerTokenObjectStoreAuth  # This will be a union
"""Any accepted object store authentication scheme."""
