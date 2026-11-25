---
title: URDF material colors match RViz
hidden: true
type: misc
---

### URDF material colors match RViz

The `rgba` of a URDF material is now used as an sRGB color, the way RViz displays it, instead of being converted from linear space.
Colored URDF models therefore look darker than before, and match what RViz shows.
