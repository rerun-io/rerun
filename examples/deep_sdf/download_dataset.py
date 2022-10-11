#!/usr/bin/env python3

import io
import os
import zipfile
from pathlib import Path

import requests


def download_mcguire_sample(package: str, name: str) -> None:
    # To satisfy Apache mod_security thing.
    headers = {
        "User-Agent": "Wget/1.12 (cygwin)",
        "Accept": "*/*",
        "Connection": "Keep-Alive",
    }
    url = f"https://casual-effects.com/g3d/data10/{package}/model/{name}/{name}.zip"

    dir = Path(os.path.dirname(__file__)).joinpath("dataset")
    os.makedirs(dir, exist_ok=True)

    def download_and_extract(url: str, path: str) -> None:
        if not os.path.exists(path):
            print(f"downloading {url}…")
            resp = requests.get(url, stream=True, headers=headers)
            z = zipfile.ZipFile(io.BytesIO(resp.content))
            z.extractall(path)

    download_and_extract(url, f"{dir}/{name}")


download_mcguire_sample("research", "buddha")
download_mcguire_sample("research", "bunny")
download_mcguire_sample("research", "dragon")
download_mcguire_sample("common", "mori_knob")


def download_glb_sample(name: str) -> None:
    url = f"https://github.com/KhronosGroup/glTF-Sample-Models/blob/master/2.0/{name}/glTF-Binary/{name}.glb?raw=true"

    dir = Path(os.path.dirname(__file__)).joinpath("dataset")
    os.makedirs(dir, exist_ok=True)

    def download_and_extract(url: str, path: str) -> None:
        if not os.path.exists(path):
            print(f"downloading {url}…")
            resp = requests.get(url, stream=True)
            with open(path, "wb+") as f:
                f.write(resp.content)

    name = name.lower()
    download_and_extract(url, f"{dir}/{name}.glb")


download_glb_sample("Avocado")
download_glb_sample("Lantern")
