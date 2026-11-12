from __future__ import annotations

from dataclasses import dataclass
from typing import TYPE_CHECKING, Literal

from rerun_bindings import ChunkIndexingPolicyInternal

if TYPE_CHECKING:
    from ._schema import Schema


@dataclass(frozen=True)
class ChunkIndexingPolicy:
    """
    Which entity paths and timelines a dataset indexes for chunk queries.

    `entity_paths` and `timelines` each take either:
    - `"all"`: every entry, including names logged later.
    - A list of names: only those. An empty list indexes nothing.

    Timelines are exact names.
    Entity paths may also be wildcard filter rules as in `filter_contents`, such as `/world/**` or `-/world/raw/**`.
    They have to be expanded with [`ChunkIndexingPolicy.match`][rerun.catalog.ChunkIndexingPolicy.match]
    before setting the policy.
    Wildcards are resolved when `match` is called, so it will not cover newly logged entity paths.
    `"all"` does cover entity paths logged later.

    Only Rerun Hub applies the policy. The OSS server always indexes everything.
    In the indexed scope, a query over every entity path returns only the indexed ones.
    See [`DatasetQueryScope`][rerun.catalog.DatasetQueryScope] for reading data the policy excludes.

    Examples
    --------
    Index everything, which is also what a dataset without a policy does:
    ```python
    dataset.set_chunk_indexing_policy(ChunkIndexingPolicy("all", "all"))
    ```

    Index every entity path, but only on `log_time`:
    ```python
    dataset.set_chunk_indexing_policy(ChunkIndexingPolicy("all", ["log_time"]))
    ```

    Index everything under `/world` except `/world/raw`, on every timeline, and wait until the
    registered data is re-indexed:
    ```python
    policy = ChunkIndexingPolicy(["/world/**", "-/world/raw/**"], "all")
    dataset.set_chunk_indexing_policy(policy.match(dataset.schema())).wait()
    ```

    """

    entity_paths: list[str] | Literal["all"]
    timelines: list[str] | Literal["all"]

    def match(self, schema: Schema) -> ChunkIndexingPolicy:
        """
        Resolve entity path rules against `schema`.

        Returns the policy unchanged if `entity_paths` is `"all"`.
        Keeps exact paths that `schema` lacks, so the policy can name entity paths logged later.

        Parameters
        ----------
        schema
            The schema to match against, usually `dataset.schema()`.

        """

        if self.entity_paths == "all":
            return self
        entity_paths = ChunkIndexingPolicyInternal.match_entity_paths(schema._internal, self.entity_paths)
        return ChunkIndexingPolicy(entity_paths=entity_paths, timelines=self.timelines)
