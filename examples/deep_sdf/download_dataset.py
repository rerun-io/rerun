#!/usr/bin/env python3

import io
import os
import requests
import zipfile


# To satisfy Apache mod_security thing.
headers = {
    "User-Agent": "Wget/1.12 (cygwin)",
    "Accept": "*/*",
    "Connection": "Keep-Alive",
}


def download_and_extract(url, path):
    if not os.path.exists(path):
        print(f"downloading {url}…")
        resp = requests.get(url, stream=True, headers=headers)
        z = zipfile.ZipFile(io.BytesIO(resp.content))
        z.extractall(path)


def download_dataset(name):
    url = f"https://casual-effects.com/g3d/data10/research/model/{name}/{name}.zip"

    dir = f"dataset"
    os.makedirs(dir, exist_ok=True)

    download_and_extract(url, f"{dir}/{name}")


download_dataset("buddha")
download_dataset("bunny")
download_dataset("dragon")
