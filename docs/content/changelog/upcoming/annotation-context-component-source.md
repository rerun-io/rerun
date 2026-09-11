---
title: "Annotation context is a visualizer component source"
hidden: true
type: feature
---

### Annotation context is a visualizer component source

Previously, the visualizer UI around annotation context was fairly inconsistent and confusing.
The visualizer component-mapping UI now shows when a field uses annotation context and allows users to opt in or out explicitly.

TODO(andreas): Add screenshot.

Blueprints can also request annotation context explicitly:

```python
import rerun as rr
import rerun.blueprint as rrb
from rerun.blueprint.encodings import ComponentSourceKind, VisualizerComponentMapping

view = rrb.Spatial2DView(
    overrides={
        "points": rr.Points2D.from_fields().visualizer(
            mappings=[
                VisualizerComponentMapping(
                    target="Points2D:colors",
                    source_kind=ComponentSourceKind.AnnotationContext,
                ),
            ]
        ),
    }
)
```
For a general overview of component mappings see [component mappings](../howto/visualization/component-mappings.md).
For a guide about annotation context see [annotation context](../concepts/visualization/annotation-context.md).

Under the hood we now resolve annotation context more rigoriously & consistently,
which led to some subtle changes in behavior, see TODO(andreas): insert link.

