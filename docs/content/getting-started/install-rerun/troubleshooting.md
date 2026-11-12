---
title: Troubleshooting
order: 500
description: Fix common problems with installing Rerun, starting the Viewer, graphics, rendering, loading .rrd files, and the web Viewer
---

You can set `RUST_LOG=debug` before running to get some verbose logging output.

If you run into any issues don't hesitate to [open a ticket](https://github.com/rerun-io/rerun/issues/new/choose)
or [join our Discord](https://discord.gg/Gcm8BbTaAj).

If you logged data but can't see it in the Viewer, see [Some of my data is not shown](../../viewer/performance/missing-data.md).

## My code broke after upgrading Rerun

Rerun does not have a stable API yet, so most releases have some breaking changes.
See the [migration guides](../../reference/migration.md) for what changed and how to update your code.

Use the same version of the SDK and the Viewer.
An older Viewer may not be able to show data from a newer SDK.

## `pip install` issues

If you see the following when running `pip install rerun-sdk` or `pip install rerun-notebook`:

```sh
ERROR: Could not find a version that satisfies the requirement rerun-sdk (from versions: none)
ERROR: No matching distribution found for rerun-sdk
```

Then `pip` could not find a wheel for your Python version and platform.
Check these, in order:

1. Your Python version: `rerun-sdk` needs Python 3.11 or newer.
   Check with `python --version`.
2. Your platform: there are wheels for Linux (x86_64 and aarch64, glibc 2.28 or newer), Windows (x86_64), and macOS on Apple silicon.
   There are no wheels for Intel Macs, or for Linux distributions with an older glibc, such as Ubuntu 18.04.
   See the full [requirements](python.md#requirements).
3. Your `pip` version: a very old `pip` can fail to find wheels that exist.
   Check the version with `pip --version`.
   If you're running a version of pip 20 or older, you should upgrade it with `pip install --upgrade pip`.

> [!WARNING]
> Depending on your system configuration this may upgrade the pip installation aliased by `pip3` instead of `pip`.

If `import rerun` fails after a successful install, make sure that you have NumPy 2 or newer installed, and that you run the same Python environment that you installed into.

## Running on Linux

The prebuilt `rerun` binaries and the `rerun-sdk` wheels should work out of the box on most Linux desktops.
If the Viewer does not start, first make sure your graphics drivers are up to date (see [Graphics issues](#graphics-issues)).
Installing `libxkbcommon-x11-0` also helps on some systems.

[TODO(#1250)](https://github.com/rerun-io/rerun/issues/1250): Running with the wayland window manager sometimes causes Rerun to crash.
Try unsetting the wayland display (`unset WAYLAND_DISPLAY` or `WAYLAND_DISPLAY= `) as a workaround.

### Building from source on Linux

You only need these packages if you build Rerun from source, for example with `cargo install rerun-cli` or as a dependency of your Rust project.
On Ubuntu and Debian:

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

On Fedora Rawhide:

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

## Startup issues

If Rerun is having trouble starting, you can try resetting its memory with:

```
rerun reset
```

## Graphics issues

<!-- This section is linked to from `crates/top/re_viewer/src/native.rs` -->

Make sure to keep your graphics drivers updated.

[Wgpu](https://github.com/gfx-rs/wgpu) (the graphics API we use) maintains a list of
[known driver issues](https://github.com/gfx-rs/wgpu/wiki/Known-Driver-Issues) and workarounds for them.

The native Viewer supports these graphics backends:

* Vulkan: the default on Linux and Windows.
* Metal: the default on macOS.
* OpenGL: a fallback on Linux and Windows, used when Vulkan is not available.

DirectX 12 is not supported.

The web Viewer uses WebGPU in Chromium-based browsers, and falls back to WebGL if WebGPU is not available.
In Safari and Firefox it always uses WebGL.

The configuration we use for wgpu can be influenced in the following ways:

-   pass `--renderer=<backend>` on startup: `<backend>` must be one of `vulkan`, `metal` or `gl` for native and
    either `webgl` or `webgpu` for the web viewer (see also `--web-viewer` argument).
    Naturally, support depends on your OS: `metal` only works on macOS, and `gl` does not work on macOS.
    -   For instance, you can try `rerun --renderer=gl` or for the web viewer respectively `rerun --web-viewer --renderer=webgl`.
    -   Alternatively, for the native viewer you can also use the `WGPU_BACKEND` environment variable with the above values.
    -   The web viewer is configured by the `renderer=<backend>` url argument, e.g. [https://rerun.io/viewer?renderer=webgl](https://rerun.io/viewer?renderer=webgl)
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

### Virtual machines and machines without a GPU

The native Viewer needs Vulkan, Metal, or OpenGL.
Some virtual machines only provide a DirectX 12 driver (for example some Windows VMs on Apple silicon), and the native Viewer can't start there.
In that case, use the web Viewer instead: `rerun --web-viewer` runs it in your browser, which uses its own graphics stack.

On Linux machines without a GPU, install a software Vulkan driver.
On Ubuntu and Debian, the `mesa-vulkan-drivers` package includes one (lavapipe).
Rendering on the CPU is slow, but it works.
Prefer software Vulkan over software OpenGL (llvmpipe): the Viewer warns about software OpenGL rasterizers because of known crashes.

## Rendering issues

### Points or lines are invisible, or far too big

Radii are in scene units by default.
If your scene is very small (for example millimeters stored as meters) or very large, a radius in scene units can be too small to see or so large that it covers everything.
Use UI points instead, which have the same size on screen no matter how far you zoom:

```python
rr.log("points", rr.Points3D(positions, radii=rr.Radius.ui_points(2.0)))
```

Negative radii are interpreted as UI points, so `rr.Radius.ui_points(2.0)` is the same as `-2.0`.
If you don't set a radius, the Viewer uses a small size in UI points.

### 2D elements flicker or hide each other

In 2D views, overlapping elements are drawn in the order given by their [`DrawOrder`](../../reference/types/components/draw_order.md).
Higher values are drawn on top.
Entities with the same draw order are drawn in an undefined order.
Give overlapping entities different draw orders, for example via the `draw_order` argument of `Points2D`, `Boxes2D`, or `Image`.

### An entity is not shown at all

If an entity's transform frame is not connected to the view's origin, the view can't place it.
The view then reports an error for that entity and doesn't draw it.
Use the transform debugging panel to look for disconnected frames, see [Debugging transform trees](../../logging-and-ingestion/data-model/transforms.md#debugging-transform-trees).

For more reasons why data may be missing, see [Some of my data is not shown](../../viewer/performance/missing-data.md).

## Loading `.rrd` files fails

### The file is from an older version of Rerun

Rerun can load `.rrd` files from version 0.23 and newer.
Files from before 0.23 are rejected with an error like `Data from Rerun version 0.22.1, which is incompatible with the local Rerun version …`.
To view such a file, use the version of Rerun that wrote it.

Files from 0.23 and newer are migrated when you load them.
To migrate them once and for all, run:

```sh
rerun rrd migrate my_recording.rrd
```

This keeps a copy of the original as `my_recording.backup.rrd`.

### The file is from a newer version of Rerun

A file written by a newer version of Rerun may load, but the Viewer warns that it `may contain data that is not compatible with this version of Rerun`.
Update the Viewer to the version of the SDK that wrote the file.

### The file is damaged or incomplete

If the program that wrote the file was killed, the end of the file can be incomplete.
Use `--continue-on-error` to save what can be read into a new file:

```sh
rerun rrd merge --continue-on-error my_recording.rrd -o my_recording_fixed.rrd
```

Use `rerun rrd verify` to check a file.
See [Work with `.rrd` files](../../logging-and-ingestion/chunks-and-rrd/manipulate-rrd-files.md) for more ways to inspect and change `.rrd` files.

## The web Viewer only shows the welcome page

If you use `rr.serve_grpc()` together with `rr.serve_web_viewer()`, and the web Viewer shows the welcome page instead of your data, check the following:

* Pass the URL of the gRPC server to the web Viewer:

  ```python
  server_uri = rr.serve_grpc()
  rr.serve_web_viewer(connect_to=server_uri)
  ```

* `connect_to` is only used for the browser tab that `serve_web_viewer` opens.
  With `open_browser=False`, or if you open the page yourself, add the gRPC server to the page URL as the `url` parameter:
  `http://localhost:9090/?url=rerun%2Bhttp%3A%2F%2Flocalhost%3A9876%2Fproxy`.
  See [Viewer URLs](../../viewer/extend/urls.md#in-the-web-viewer).
* If the browser runs on another machine, use the host name of the machine that runs your program instead of `localhost`, and make sure that both the web Viewer port (9090) and the gRPC port (9876) are reachable from the browser.
* Keep your program running.
  Both servers stop when the Python process exits.

## Video stuttering

On some browsers the default video decoder may cause stuttering.
This has been for instance observed with Chrome 129 on Windows.

To mitigate these issues, you can try to specify software decoding.
This can be configured from the viewer's option menu. Alternatively, you can also override this setting on startup:
* for the web viewer pass `&video_decoder=prefer_software` as a url parameter
* for the native viewer & for starting the web viewer via command line (`--web-viewer` argument), pass `--video-decoder=prefer_software`

For more information about video decoding, see also the reference page on [video](../../logging-and-ingestion/media/video.md).
