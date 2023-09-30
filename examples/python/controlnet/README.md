---
title: ControlNet
python: https://github.com/rerun-io/rerun/tree/latest/examples/python/controlnet/main.py
tags: [controlnet, canny, huggingface, stable-diffusion, tensor, text]
thumbnail: https://static.rerun.io/depth_guided_stable_diffusion/a85516aba09f72649517891d767e15383ce7f4ea/480w.png
thumbnail_dimensions: [480, 253]
---

This example integrates Rerun into [Hugging Face's ControlNet example](https://huggingface.co/docs/diffusers/using-diffusers/controlnet#controlnet).

To run this example use
```bash
pip install -r examples/python/controlnet/requirements.txt
python examples/python/controlnet/main.py
```

You can specify your own prompts and image using
```bash
main.py [--img_path IMG_PATH] [--prompt PROMPT] [--negative_prompt NEGATIVE_PROMPT]
```
