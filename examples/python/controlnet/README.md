<--[metadata]
title = "ControlNet"
tags = ["controlnet", "canny", "huggingface", "stable-diffusion", "tensor", "text"]
thumbnail = "https://static.rerun.io/controlnet/8aace9c59a423c2eeabe4b7f9abb5187559c52e8/480w.png"
thumbnail_dimensions = [480, 303]
-->


This example integrates Rerun into [Hugging Face's ControlNet example](https://huggingface.co/docs/diffusers/using-diffusers/controlnet#controlnet). ControlNet allows to condition Stable Diffusion on various modalities. In this example we condition on edges detected by the Canny edge detector.

https://vimeo.com/870289439?autoplay=1&loop=1&autopause=0&background=1&muted=1&ratio=1440:1080

To run this example use
```bash
pip install -r examples/python/controlnet/requirements.txt
python examples/python/controlnet/main.py
```

You can specify your own image and prompts using
```bash
main.py [--img-path IMG_PATH] [--prompt PROMPT] [--negative-prompt NEGATIVE_PROMPT]
```