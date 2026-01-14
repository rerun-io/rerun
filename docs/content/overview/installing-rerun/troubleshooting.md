---
title: Troubleshooting
order: 500
---

You can set `RUST_LOG=debug` before running to get some verbose logging output.

If you run into any issues don't hesitate to [open a ticket](https://github.com/rerun-io/rerun/issues/new/choose)
or [join our Discord](https://discord.gg/Gcm8BbTaAj).

## Running on Linux

Rerun should work out-of-the-box on Mac and Windows, but on Linux you need to first run:

```sh
sudo apt-get -y install \
    libclang-dev \
    libatk-bridge2.0 \
    libfontconfig1-dev \
    libfreetype6-dev \
    libglib2.0-dev \
    libgtk-3-dev \
    libssl-dev \
    libxcb-render0-dev \
    libxcb-shape0-dev \
    libxcb-xfixes0-dev \
    libxkbcommon-dev \
    patchelf
```

On Fedora Rawhide you need to run:

```sh
sudo dnf install \
    clang \
    clang-devel \
    clang-tools-extra \
    libxcb-devel \
    libxkbcommon-devel \
    openssl-devel \
    pkg-config
```

[TODO(#1250)](https://github.com/rerun-io/rerun/issues/1250): Running with the wayland window manager
sometimes causes Rerun to crash. Try unsetting the wayland display (`unset WAYLAND_DISPLAY` or `WAYLAND_DISPLAY= `) as a workaround.

## Running on WSL2 (Ubuntu)

WSL's graphics drivers won't work out of the box and you'll have to update to a more recent version.
To install the latest stable version of the mesa Vulkan drivers run:
```sh
sudo add-apt-repository ppa:kisak/kisak-mesa
sudo apt-get update
sudo apt-get install -y mesa-vulkan-drivers
```

Since the Mesa driver on WSL dispatches to the Windows host graphics driver, it is important to keep the Windows drivers up-to-date as well.
For example, [line rendering issues](https://github.com/rerun-io/rerun/issues/6749) have been observed when running from WSL with an
outdated AMD driver on the Windows host.

On Ubuntu 24 [issues with Wayland](https://github.com/rerun-io/rerun/issues/6748) have been observed.
To mitigate this install `libxkbcommon-x11`
```
sudo apt install libxkbcommon-x11-0
```
And unset the wayland display either by `unset WAYLAND_DISPLAY` or `WAYLAND_DISPLAY= `.

## `pip install` issues

If you see the following when running `pip install rerun-sdk` or `pip install rerun-notebook` on a supported platform:

```sh
ERROR: Could not find a version that satisfies the requirement rerun-sdk (from versions: none)
ERROR: No matching distribution found for rerun-sdk
```

Then this is likely because you're running a version of pip that is too old.
You can check the version of pip with `pip --version`.
If you're running a version of pip 20 or older, you should upgrade it with `pip install --upgrade pip`.
⚠️ depending on your system configuration this may upgrade the pip installation aliased by `pip3` instead of `pip`.


## Startup issues

If Rerun is having trouble starting, you can try resetting its memory with:

```
rerun reset
```

## Graphics issues

<!-- This section is linked to from `crates/viewer/re_viewer/src/native.rs` -->

Make sure to keep your graphics drivers updated.

[Wgpu](https://github.com/gfx-rs/wgpu) (the graphics API we use) maintains a list of
[known driver issues](https://github.com/gfx-rs/wgpu/wiki/Known-Driver-Issues) and workarounds for them.

The configuration we use for wgpu can be influenced in the following ways:

-   pass `--renderer=<backend>` on startup: `<backend>` must be one of `vulkan`, `metal` or `gl` for native and
    either `webgl` or `webgpu` for the web viewer (see also `--web-viewer` argument).
    Naturally, support depends on your OS. The default backend is `vulkan` everywhere except on Mac where we use `metal`.
    On the web we prefer WebGPU and fall back automatically to WebGL if no support for WebGPU was detected.
    -   For instance, you can try `rerun --renderer=gl` or for the web viewer respectively `rerun --web-viewer --renderer=webgl`.
    -   Alternatively, for the native viewer you can also use the `WGPU_BACKEND` environment variable with the above values.
    -   The web viewer is configured by the `renderer=<backend>` url argument, e.g. [https://rerun.io/viewer?renderer=webgl]
-   `WGPU_POWER_PREF`: Overwrites the power setting used for choosing a graphics adapter, must be `high` or `low`. (Default is `high`)

We recommend setting these only if you're asked to try them or know what you're doing,
since we don't support all of these settings equally well.

### Multiple GPUs

When using Wgpu's Vulkan backend (the default on Windows & Linux) on a computer that has both integrated and dedicated GPUs, a lot of issues can arise from Vulkan either picking the "wrong" GPU at runtime, or even simply from the fact that this choice conflicts with other driver picking technologies (e.g. NVIDIA Optimus).

In both cases, forcing Vulkan to pick either the integrated or discrete GPU (try both!) using the [`VK_ICD_FILENAMES`](https://vulkan.lunarg.com/doc/view/latest/mac/LoaderDriverInterface.html#user-content-driver-discovery) environment variable might help with crashes, artifacts and bad performance. E.g.:

-   Force the Intel integrated GPU:
    -   Linux: `export VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/intel.json`.
-   Force the discrete Nvidia GPU:
    -   Linux: `export VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/nvidia.json`.
    -   Windows: `set VK_ICD_FILENAMES=\windows\system32\nv-vk64.json`.

## Video stuttering

On some browsers the default video decoder may cause stuttering.
This has been for instance observed with Chrome 129 on Windows.

To mitigate these issues, you can try to specify software decoding.
This can be configured from the viewer's option menu. Alternatively, you can also override this setting on startup:
* for the web viewer pass `&video_decoder=prefer_software` as a url parameter
* for the native viewer & for starting the web viewer via command line (`--web-viewer` argument), pass `--video-decoder=prefer_software`

For more information about video decoding, see also the reference page on [video](../../concepts/logging-and-ingestion/video.md).
