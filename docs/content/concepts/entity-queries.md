---
title: Entity Queries
order: 9
---

TODO(jleibs): flesh this out with more details/examples

The contents of a space view are found by combining a collection of `QueryExpression`s.

```diff
+ /world/**           # add everything…
- /world/roads/**     # …but remove all roads…
+ /world/roads/main   # …but show main road
```

-   If there is multiple matching rules, the most specific rule wins.
-   If there are multiple rules of the same specificity, the last one wins.
-   If no rules match, the path is excluded.

The`/**` suffix matches the whole subtree, i.e. self and any child, recursively
(`/world/**`matches both`/world`and`/world/car/driver`).

Other uses of `*` are not yet supported.
