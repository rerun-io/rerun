---
title: Improved viewer performance with many entities and views
hidden: true
type: feature
---

### Improved viewer performance with many entities and views

The viewer now renders recordings with many entities and blueprints with many views faster.
The more views a blueprint has, the bigger the gain.

| Entities | Views | Before   | After    | Speedup |
|----------|-------|----------|----------|---------|
| 940      | 8     | 3.6 ms   | 2.5 ms   | 1.4×    |
| 940      | 32    | 4.7 ms   | 2.8 ms   | 1.7×    |
| 940      | 128   | 8.5 ms   | 4.1 ms   | 2.1×    |
| 4,700    | 8     | 18.2 ms  | 13.5 ms  | 1.3×    |
| 4,700    | 32    | 19.9 ms  | 14.0 ms  | 1.4×    |
| 4,700    | 128   | 27.2 ms  | 15.6 ms  | 1.7×    |
| 18,800   | 8     | 84.5 ms  | 66.8 ms  | 1.3×    |
| 18,800   | 32    | 95.2 ms  | 67.9 ms  | 1.4×    |
| 18,800   | 128   | 130 ms   | 69.7 ms  | 1.9×    |

*Median frame time of a viewer with views in tabs, measured on an Apple M4 with 10 cores.*

The viewer still has many issues with many entities, specifically with some UI open, like the time panel. But this change, caching some queries that were done each frame before, is a great step towards handling many entities smoothly.
