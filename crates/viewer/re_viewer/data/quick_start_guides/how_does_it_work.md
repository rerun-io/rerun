## How does it work?

Rerun's goal is to make handling and visualizing multimodal data streams easy and performant.

Rerun is made of two main building blocks: the SDK and the Viewer. The data provided by the user code is serialized by the SDK and transferred (via a log file, a TCP socket, a WebSocket, etc.) to the Viewer process for visualization. You can learn more about Rerun's operating modes [here](https://www.rerun.io/docs/reference/sdk-operating-modes).

In the example above, the SDK connects via a TCP socket to the viewer.

The `log()` function logs _entities_ represented by the "entity path" provided as first argument. Entities are a collection of _components_, which hold the actual data such as position, color, or pixel data. _Archetypes_ such as `Points3D` are builder objects which help creating entities with a consistent set of components that are recognized by the Viewer (they can be entirely bypassed when required by advanced use-cases). You can learn more about Rerun's data model [here](https://www.rerun.io/docs/concepts/entity-component).
