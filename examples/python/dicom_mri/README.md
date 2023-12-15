---
title: Dicom MRI
tags: [tensor, mri, dicom]
description: "Example using a DICOM MRI scan. This demonstrates the flexible tensor slicing capabilities of the Rerun viewer."
thumbnail: https://static.rerun.io/dicom_mri/e39f34a1b1ddd101545007f43a61783e1d2e5f8e/480w.png
thumbnail_dimensions: [480, 285]
channel: main
---

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/dicom_mri/e39f34a1b1ddd101545007f43a61783e1d2e5f8e/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/dicom_mri/e39f34a1b1ddd101545007f43a61783e1d2e5f8e/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/dicom_mri/e39f34a1b1ddd101545007f43a61783e1d2e5f8e/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/dicom_mri/e39f34a1b1ddd101545007f43a61783e1d2e5f8e/1200w.png">
  <img src="https://static.rerun.io/dicom_mri/e39f34a1b1ddd101545007f43a61783e1d2e5f8e/full.png" alt="">
</picture>

Example using a [DICOM](https://en.wikipedia.org/wiki/DICOM) MRI scan. This demonstrates the flexible tensor slicing capabilities of the Rerun viewer.

```bash
pip install -r examples/python/dicom_mri/requirements.txt
python examples/python/dicom_mri/main.py
```
