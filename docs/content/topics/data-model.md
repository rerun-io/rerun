---
title: Data model
order: 100
---

Understanding Rerun's data model is fundamental to effectively logging and querying data.

Rerun uses an Entity-Component-System (ECS) inspired data model where:
- **Entities** are the "things" you log, identified by entity paths
- **Components** contain the actual data associated with entities
- **Timelines** track when data was logged

This section covers:
- [Entities and Components](data-model/entity-component.md): The core data model concepts
- [Entity Path](data-model/entity-path.md): How entities are identified and organized
- [Events and Timelines](data-model/timelines.md): How temporal data is structured
- [Static Data](data-model/static.md): Non-temporal data handling
- [Transforms & Coordinate Frames](data-model/transforms.md): Spatial relationships
- [Batches](data-model/batches.md): Efficient data logging
- [Chunks](data-model/chunks.md): Internal data organization
- [Video](data-model/video.md): Video data handling
