---
title: ControlNet
python: https://github.com/rerun-io/rerun/tree/latest/examples/python/controlnet/main.py
tags: [controlnet, canny, huggingface, stable-diffusion, tensor, text]
thumbnail: https://static.rerun.io/controlnet/cec698ef2ee9d9bf24e3d3c3fcd366d48f993915/480w.png
thumbnail_dimensions: [480, 298]
---

This example integrates Rerun into [Hugging Face's ControlNet example](https://huggingface.co/docs/diffusers/using-diffusers/controlnet#controlnet).

https://vimeo.com/869834443?autoplay=1&loop=1&autopause=0&background=1&muted=1&ratio=100:63.14

To run this example use
```bash
pip install -r examples/python/controlnet/requirements.txt
python examples/python/controlnet/main.py
```

You can specify your own image and prompts using
```bash
main.py [--img_path IMG_PATH] [--prompt PROMPT] [--negative_prompt NEGATIVE_PROMPT]
```
