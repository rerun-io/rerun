# Rerun Examples
Each example comes with its own set of dependencies listed in a `requirements.txt` file. For example, to install dependencies and run the toy `car` example (which doesn't need to download any data) run:

```sh
pip install -r examples/car/requirements.txt
examples/car/main.py
```

You can also install all dependencies needed to run all examples with:

```sh
pip install -r examples/requirements.txt
```

> Note: The `stable_diffusion` example requires installing `diffusers` and `transformers` directly from main. To install run `pip install -U -r examples/stable_diffusion/requirements.txt`.

## Contributions welcome
Feel free to open a PR to add a new example!

See [`CONTRIBUTING.md`](../CONTRIBUTING.md) for details on how to contribute.
