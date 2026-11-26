---
title: "Reduce iterable dataloader scheduling memory"
hidden: true
type: misc
---

### Reduce iterable dataloader scheduling memory

`BlockShuffle` and `NoShuffle` now use substantially less worker memory when iterating large datasets.
This improves startup memory use for ordinary and curated iterable datasets.
