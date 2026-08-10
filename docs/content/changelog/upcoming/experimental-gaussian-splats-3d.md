---
title: "Experimental 3D gaussian splat support"
hidden: true
type: highlight
---

### Experimental 3D gaussian splat support

Rerun has now an experimental [`GaussianSplats3D`](../reference/types/archetypes/gaussian_splats3d.md) archetype and supports visualizing them in the Viewer!

<video width="100%" autoplay loop muted controls>
    <source src="https://static.rerun.io/2ef9455946158cc698f679b72782ddc9a8c8ff44_splat-training.mp4" type="video/mp4" />
</video>

You can load any gaussian splat PLY file directly with the viewer or log the archetype directly from the logging SDK.
The archetype supports anisotropic scale and rotation, opacity, and view-dependent color using spherical harmonics up to degree 3.

This support is experimental as the archetype and our renderer (which is kept very simple for now and is compatible with all browsers) may still evolve significantly.

Let us know which formats and splatting variants you would like us to support next!
