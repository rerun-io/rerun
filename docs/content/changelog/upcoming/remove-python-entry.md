---
title: Removed deprecated Python catalog entry methods and single-URI registration shorthand
hidden: true
type: breaking
---

Rerun 0.44 removes three Python catalog APIs:

- `Entry.update()`, deprecated in 0.29: use `entry.set_name(name)` instead of `entry.update(name=name)`.
- The single-string form of `DatasetEntry.register()`, deprecated in 0.32: pass a list even for one recording, as in `dataset.register([uri])`.
- `DatasetEntry.manifest()`, deprecated in 0.34: this method was for internal diagnostics and has no public replacement.
