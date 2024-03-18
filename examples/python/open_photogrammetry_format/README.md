<!--[metadata]
title = "Open Photogrammetry Format"
tags = ["2D", "3D", "camera", "photogrammetry"]
description = "Displays a photogrammetrically reconstructed 3D point cloud loaded from an Open Photogrammetry Format (OPF) file."
thumbnail = "https://static.rerun.io/open_photogrammetry_format/603d5605f9670889bc8bce3365f16b831fce1eb1/480w.png"
thumbnail_dimensions = [480, 310]
channel = "release"
build_args = ["--jpeg-quality=50"]
-->

<picture data-inline-viewer="open_photogrammetry_format">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/open_photogrammetry_format/603d5605f9670889bc8bce3365f16b831fce1eb1/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/open_photogrammetry_format/603d5605f9670889bc8bce3365f16b831fce1eb1/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/open_photogrammetry_format/603d5605f9670889bc8bce3365f16b831fce1eb1/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/open_photogrammetry_format/603d5605f9670889bc8bce3365f16b831fce1eb1/1200w.png">
  <img src="https://static.rerun.io/open_photogrammetry_format/603d5605f9670889bc8bce3365f16b831fce1eb1/full.png" alt="">
</picture>


Uses [`pyopf`](https://github.com/Pix4D/pyopf) to load and display a photogrammetrically reconstructed 3D point cloud in the [Open Photogrammetry Format (OPF)](https://www.pix4d.com/open-photogrammetry-format/).


```bash
pip install -r examples/python/open_photogrammetry_format/requirements.txt
python examples/python/open_photogrammetry_format/main.py
```

Requires Python 3.10 or higher because of [`pyopf`](https://pypi.org/project/pyopf/).
