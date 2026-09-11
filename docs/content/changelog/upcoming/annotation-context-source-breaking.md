---
title: "Recorded labels no longer mix with annotation labels"
hidden: true
type: breaking
---

### Recorded labels no longer mix with annotation labels

Annotation labels no longer fill gaps in partial recorded label batches.
For example:

```python
import rerun as rr

rr.init("rerun_example_annotation_source", spawn=True)
rr.log("/", rr.AnnotationContext([(1, "car")]), static=True)
rr.log(
    "points",
    rr.Points2D(
        [[0, 0], [1, 0], [2, 0]],
        class_ids=[1, 1, 1],
        labels=["first", "second"],
        show_labels=True,
    ),
)
```

Previously, the third point was labeled `car`; now it is unlabeled.
Log `labels=["first", "second", "car"]` to preserve the previous result, or omit recorded labels to use annotations for all points.

Docs: ../concepts/visualization/annotation-context.md
