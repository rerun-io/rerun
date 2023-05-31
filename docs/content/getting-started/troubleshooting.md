---
title: Troubleshooting
order: 7
---

You can set `RUST_LOG=debug` before running to get some verbose logging output.

If you run into any issues don't hesitate to [open a ticket](https://github.com/rerun-io/rerun/issues/new/choose)
or [join our Discord](https://discord.gg/Gcm8BbTaAj).

## Running on Linux
Rerun should work out-of-the-box on Mac and Windows, but on Linux you need to first run:

`sudo apt-get install -y libclang-dev libgtk-3-dev libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev libxkbcommon-dev libssl-dev`

On Fedora Rawhide you need to run:

`dnf install clang clang-devel clang-tools-extra libxkbcommon-devel pkg-config openssl-devel libxcb-devel`

On WSL2, in addition to the above packages for Linux, you also need to run:

`sudo apt-get install -y libvulkan1 libxcb-randr0 mesa-vulkan-drivers adwaita-icon-theme-full`

[TODO(#1250)](https://github.com/rerun-io/rerun/issues/1250): Running with the wayland window manager 
sometimes causes Rerun to crash. Try setting `WINIT_UNIX_BACKEND=x11` as a workaround.

## Graphics issues

[Wgpu](https://github.com/gfx-rs/wgpu) (the graphics API we use) maintains a list of
[known driver issues](https://github.com/gfx-rs/wgpu/wiki/Known-Driver-Issues) and workarounds for them.

The following environment variables overwrite the config we choose for wgpu:
* `WGPU_BACKEND`: Overwrites the graphics backend used, must be one of `vulkan`, `metal`, `dx12`, `dx11`, or `gl`.
    Naturally, support depends on your OS. Default is `vulkan` everywhere except on Mac where we use `metal`.
* `WGPU_POWER_PREF`: Overwrites the power setting used for choosing a graphics adapter, must be `high` or `low`. (Default is `high`)

We recommend setting these only if you're asked to try them or know what you're doing,
since we don't support all of these settings equally well.

### Multiple GPUs

When using Wgpu's Vulkan backend (the default on Windows & Linux) on a computer that has both integrated and dedicated GPUs, a lot of issues can arise from Vulkan either picking the "wrong" GPU at runtime, or even simply from the fact that this choice conflicts with other driver picking technologies (e.g. NVIDIA Optimus).

In both cases, forcing Vulkan to pick either the integrated or discrete GPU (try both!) using the [`VK_ICD_FILENAMES`](https://vulkan.lunarg.com/doc/view/1.3.204.1/mac/LoaderDriverInterface.html#user-content-driver-discovery) environment variable might help with crashes, artifacts and bad performance. E.g.:
- Force the Intel integrated GPU:
  - Linux: `export VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/intel.json`.
- Force the discrete Nvidia GPU:
  - Linux: `export VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/nvidia.json`.
  - Windows: `set VK_ICD_FILENAMES=\windows\system32\nv-vk64.json`.
