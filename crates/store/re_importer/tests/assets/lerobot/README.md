# LeRobot test datasets

These fixtures contain the first three episodes of the [`pollen-robotics/apple_storage`](https://huggingface.co/datasets/pollen-robotics/apple_storage) dataset.
The upstream dataset and both Rerun mirrors declare the Apache-2.0 license; see [`LICENSE-APACHE`](../../../../../../LICENSE-APACHE).

`v21_apple_storage` is derived from [`rerun/v21_apple_storage`](https://huggingface.co/datasets/rerun/v21_apple_storage) at revision `91b47932e95ae0c688960f96889bd3f447383d52`.
`v30_apple_storage` is derived from [`rerun/v30_apple_storage`](https://huggingface.co/datasets/rerun/v30_apple_storage) at revision `da288fcf2eee05e4f5839735d3c19839bf0b24e5`.

The video files are modified from the upstream snapshots by transcoding them to 320×240 H.264 with FFmpeg's `libx264` encoder at CRF 32 and a 30-frame GOP.
Their frame rate, frame count, and duration are unchanged, and the corresponding dimensions in each `meta/info.json` are updated.
The Parquet and MP4 payloads are stored using Git LFS.
