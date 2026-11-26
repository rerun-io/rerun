---
title: Removed deprecated Python transform and blueprint panel arguments
hidden: true
type: breaking
---

Rerun 0.44 removes `Transform3D`’s `from_parent` argument and the blueprint panels’ `expanded` argument, both deprecated since at least 0.18.

Replace `rr.Transform3D(from_parent=True)` with `rr.Transform3D(relation=rr.TransformRelation.ChildFromParent)`; omit `relation` for the old `False` case.
Replace `expanded=True` or `False` on blueprint panels with `state="expanded"` or `state="collapsed"`.
