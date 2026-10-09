---
title: "Update a mesh texture from an image in Python"
hidden: true
type: misc
---

In Python, `rr.Mesh3D.from_albedo_texture(image)` updates only the albedo texture of a `Mesh3D`, so a texture can change over time without logging the mesh geometry again.
