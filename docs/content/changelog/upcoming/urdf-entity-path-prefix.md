---
title: URDF entity path prefixes
hidden: true
type: breaking
---

### URDF entity path prefixes

Fixed a bug where `UrdfTree.from_file_path` treated `entity_path_prefix="world/robot"` as a single entity path part.
It now interprets the prefix as two entity path parts, matching the behavior of `log_file_from_path`.
If you rely on the old parsing behavior, pass `rr.escape_entity_path_part("world/robot")` as the prefix to preserve the single-part path.
