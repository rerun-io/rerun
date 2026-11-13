<!--[metadata]
title = "MetricTrail: metric camera tracking from one video"
description = "Track a moving camera through one uncalibrated video with NVIDIA's MetricTrail and get the camera path, the focal length and a surface in meters, step by step."
tags = ["2D", "3D", "SLAM", "Depth", "Pinhole camera", "Video", "Mesh", "Hugging Face"]
source = "https://github.com/rerun-io/examples-monorepo/tree/main/packages/metrictrail"
thumbnail = "https://static.rerun.io/metrictrail_thumbnail_768x576_8a77f49cf/e8ed3b325985585b67ad56689f2521ba092ef99d/full.png"
thumbnail_dimensions = [768, 576]
preview_rrd_url = "https://huggingface.co/datasets/pablovela5620/metrictrail-rerun-example/resolve/main/metrictrail-pool-17s.rrd"
-->

[![Open in Hugging Face Spaces](https://huggingface.co/datasets/huggingface/badges/resolve/main/open-in-hf-spaces-md.svg)](https://pablovela5620-metrictrail-rerun.hf.space)

```agent-prompt
```

Try it in your browser with no install (the Space runs MetricTrail on ZeroGPU), or copy a prompt for your coding agent to install and run it on your own Linux machine with an NVIDIA GPU.

<video src="https://static.rerun.io/ec6c5f7ac2cfe59a4fca10fe8780e3200c1e01ef_seg-run-8a77f49cf-17s.mp4" poster="https://static.rerun.io/metrictrail_seg_run_8a77f49cf/4112f266d04716aa7a688bc48d5cea57fd472a6e/full.webp" autoplay muted loop playsinline></video>

MetricTrail tracks a moving camera through one uncalibrated video and returns the camera path, the focal length and a surface, all in meters.
This example runs [NVIDIA's MetricTrail](https://research.nvidia.com/labs/sil/projects/metrictrail/) on a phone video and logs every step to Rerun: the video with the tracked patches, the metric depth, the camera path and a mesh of the scene.

## Run the code

<ul aria-label="Requirements" style="display:flex;flex-wrap:wrap;gap:6px;list-style:none;margin:20px 0 0;padding:0">
<li style="display:flex;align-items:center;gap:7px;min-height:32px;box-sizing:border-box;margin:0;padding:5px 8px;border:1px solid var(--color-border-strong, oklch(0.371 0 0));border-radius:4px;background:var(--color-code-bg, oklch(0.145 0 0));color:var(--color-fg, oklch(0.97 0 0));text-decoration:none"><svg aria-hidden="true" width="16" height="16" viewBox="0 0 24 24" fill="currentColor" style="display:block;flex:none;color:var(--color-muted, oklch(0.708 0 0))"><path d="M8.948 8.798v-1.43a6.7 6.7 0 0 1 .424-.018c3.922-.124 6.493 3.374 6.493 3.374s-2.774 3.851-5.75 3.851c-.398 0-.787-.062-1.158-.185v-4.346c1.528.185 1.837.857 2.747 2.385l2.04-1.714s-1.492-1.952-4-1.952a6.016 6.016 0 0 0-.796.035m0-4.735v2.138l.424-.027c5.45-.185 9.01 4.47 9.01 4.47s-4.08 4.964-8.33 4.964c-.37 0-.733-.035-1.095-.097v1.325c.3.035.61.062.91.062 3.957 0 6.82-2.023 9.593-4.408.459.371 2.34 1.263 2.73 1.652-2.633 2.208-8.772 3.984-12.253 3.984-.335 0-.653-.018-.971-.053v1.864H24V4.063zm0 10.326v1.131c-3.657-.654-4.673-4.46-4.673-4.46s1.758-1.944 4.673-2.262v1.237H8.94c-1.528-.186-2.73 1.245-2.73 1.245s.68 2.412 2.739 3.11M2.456 10.9s2.164-3.197 6.5-3.533V6.201C4.153 6.59 0 10.653 0 10.653s2.35 6.802 8.948 7.42v-1.237c-4.84-.6-6.492-5.936-6.492-5.936z"/></svg><span style="display:flex;flex-direction:column;line-height:16px"><span style="font-size:11.5px;color:var(--color-muted, oklch(0.708 0 0))">NVIDIA GPU</span><span style="display:none">: </span><span style="font-size:13px;color:var(--color-fg, oklch(0.97 0 0))">> 12 GB, CUDA 13</span></span></li>
<li style="display:flex;align-items:center;gap:7px;min-height:32px;box-sizing:border-box;margin:0;padding:5px 8px;border:1px solid var(--color-border-strong, oklch(0.371 0 0));border-radius:4px;background:var(--color-code-bg, oklch(0.145 0 0));color:var(--color-fg, oklch(0.97 0 0));text-decoration:none"><svg aria-hidden="true" width="16" height="16" viewBox="0 0 24 24" fill="currentColor" style="display:block;flex:none;color:var(--color-muted, oklch(0.708 0 0))"><path d="M12.504 0c-.155 0-.315.008-.48.021-4.226.333-3.105 4.807-3.17 6.298-.076 1.092-.3 1.953-1.05 3.02-.885 1.051-2.127 2.75-2.716 4.521-.278.832-.41 1.684-.287 2.489a.424.424 0 00-.11.135c-.26.268-.45.6-.663.839-.199.199-.485.267-.797.4-.313.136-.658.269-.864.68-.09.189-.136.394-.132.602 0 .199.027.4.055.536.058.399.116.728.04.97-.249.68-.28 1.145-.106 1.484.174.334.535.47.94.601.81.2 1.91.135 2.774.6.926.466 1.866.67 2.616.47.526-.116.97-.464 1.208-.946.587-.003 1.23-.269 2.26-.334.699-.058 1.574.267 2.577.2.025.134.063.198.114.333l.003.003c.391.778 1.113 1.132 1.884 1.071.771-.06 1.592-.536 2.257-1.306.631-.765 1.683-1.084 2.378-1.503.348-.199.629-.469.649-.853.023-.4-.2-.811-.714-1.376v-.097l-.003-.003c-.17-.2-.25-.535-.338-.926-.085-.401-.182-.786-.492-1.046h-.003c-.059-.054-.123-.067-.188-.135a.357.357 0 00-.19-.064c.431-1.278.264-2.55-.173-3.694-.533-1.41-1.465-2.638-2.175-3.483-.796-1.005-1.576-1.957-1.56-3.368.026-2.152.236-6.133-3.544-6.139zm.529 3.405h.013c.213 0 .396.062.584.198.19.135.33.332.438.533.105.259.158.459.166.724 0-.02.006-.04.006-.06v.105a.086.086 0 01-.004-.021l-.004-.024a1.807 1.807 0 01-.15.706.953.953 0 01-.213.335.71.71 0 00-.088-.042c-.104-.045-.198-.064-.284-.133a1.312 1.312 0 00-.22-.066c.05-.06.146-.133.183-.198.053-.128.082-.264.088-.402v-.02a1.21 1.21 0 00-.061-.4c-.045-.134-.101-.2-.183-.333-.084-.066-.167-.132-.267-.132h-.016c-.093 0-.176.03-.262.132a.8.8 0 00-.205.334 1.18 1.18 0 00-.09.4v.019c.002.089.008.179.02.267-.193-.067-.438-.135-.607-.202a1.635 1.635 0 01-.018-.2v-.02a1.772 1.772 0 01.15-.768c.082-.22.232-.406.43-.533a.985.985 0 01.594-.2zm-2.962.059h.036c.142 0 .27.048.399.135.146.129.264.288.344.465.09.199.14.4.153.667v.004c.007.134.006.2-.002.266v.08c-.03.007-.056.018-.083.024-.152.055-.274.135-.393.2.012-.09.013-.18.003-.267v-.015c-.012-.133-.04-.2-.082-.333a.613.613 0 00-.166-.267.248.248 0 00-.183-.064h-.021c-.071.006-.13.04-.186.132a.552.552 0 00-.12.27.944.944 0 00-.023.33v.015c.012.135.037.2.08.334.046.134.098.2.166.268.01.009.02.018.034.024-.07.057-.117.07-.176.136a.304.304 0 01-.131.068 2.62 2.62 0 01-.275-.402 1.772 1.772 0 01-.155-.667 1.759 1.759 0 01.08-.668 1.43 1.43 0 01.283-.535c.128-.133.26-.2.418-.2zm1.37 1.706c.332 0 .733.065 1.216.399.293.2.523.269 1.052.468h.003c.255.136.405.266.478.399v-.131a.571.571 0 01.016.47c-.123.31-.516.643-1.063.842v.002c-.268.135-.501.333-.775.465-.276.135-.588.292-1.012.267a1.139 1.139 0 01-.448-.067 3.566 3.566 0 01-.322-.198c-.195-.135-.363-.332-.612-.465v-.005h-.005c-.4-.246-.616-.512-.686-.71-.07-.268-.005-.47.193-.6.224-.135.38-.271.483-.336.104-.074.143-.102.176-.131h.002v-.003c.169-.202.436-.47.839-.601.139-.036.294-.065.466-.065zm2.8 2.142c.358 1.417 1.196 3.475 1.735 4.473.286.534.855 1.659 1.102 3.024.156-.005.33.018.513.064.646-1.671-.546-3.467-1.089-3.966-.22-.2-.232-.335-.123-.335.59.534 1.365 1.572 1.646 2.757.13.535.16 1.104.021 1.67.067.028.135.06.205.067 1.032.534 1.413.938 1.23 1.537v-.043c-.06-.003-.12 0-.18 0h-.016c.151-.467-.182-.825-1.065-1.224-.915-.4-1.646-.336-1.77.465-.008.043-.013.066-.018.135-.068.023-.139.053-.209.064-.43.268-.662.669-.793 1.187-.13.533-.17 1.156-.205 1.869v.003c-.02.334-.17.838-.319 1.35-1.5 1.072-3.58 1.538-5.348.334a2.645 2.645 0 00-.402-.533 1.45 1.45 0 00-.275-.333c.182 0 .338-.03.465-.067a.615.615 0 00.314-.334c.108-.267 0-.697-.345-1.163-.345-.467-.931-.995-1.788-1.521-.63-.4-.986-.87-1.15-1.396-.165-.534-.143-1.085-.015-1.645.245-1.07.873-2.11 1.274-2.763.107-.065.037.135-.408.974-.396.751-1.14 2.497-.122 3.854a8.123 8.123 0 01.647-2.876c.564-1.278 1.743-3.504 1.836-5.268.048.036.217.135.289.202.218.133.38.333.59.465.21.201.477.335.876.335.039.003.075.006.11.006.412 0 .73-.134.997-.268.29-.134.52-.334.74-.4h.005c.467-.135.835-.402 1.044-.7zm2.185 8.958c.037.6.343 1.245.882 1.377.588.134 1.434-.333 1.791-.765l.211-.01c.315-.007.577.01.847.268l.003.003c.208.199.305.53.391.876.085.4.154.78.409 1.066.486.527.645.906.636 1.14l.003-.007v.018l-.003-.012c-.015.262-.185.396-.498.595-.63.401-1.746.712-2.457 1.57-.618.737-1.37 1.14-2.036 1.191-.664.053-1.237-.2-1.574-.898l-.005-.003c-.21-.4-.12-1.025.056-1.69.176-.668.428-1.344.463-1.897.037-.714.076-1.335.195-1.814.12-.465.308-.797.641-.984l.045-.022zm-10.814.049h.01c.053 0 .105.005.157.014.376.055.706.333 1.023.752l.91 1.664.003.003c.243.533.754 1.064 1.189 1.637.434.598.77 1.131.729 1.57v.006c-.057.744-.48 1.148-1.125 1.294-.645.135-1.52.002-2.395-.464-.968-.536-2.118-.469-2.857-.602-.369-.066-.61-.2-.723-.4-.11-.2-.113-.602.123-1.23v-.004l.002-.003c.117-.334.03-.752-.027-1.118-.055-.401-.083-.71.043-.94.16-.334.396-.4.69-.533.294-.135.64-.202.915-.47h.002v-.002c.256-.268.445-.601.668-.838.19-.201.38-.336.663-.336zm7.159-9.074c-.435.201-.945.535-1.488.535-.542 0-.97-.267-1.28-.466-.154-.134-.28-.268-.373-.335-.164-.134-.144-.333-.074-.333.109.016.129.134.199.2.096.066.215.2.36.333.292.2.68.467 1.167.467.485 0 1.053-.267 1.398-.466.195-.135.445-.334.648-.467.156-.136.149-.267.279-.267.128.016.034.134-.147.332a8.097 8.097 0 01-.69.468zm-1.082-1.583V5.64c-.006-.02.013-.042.029-.05.074-.043.18-.027.26.004.063 0 .16.067.15.135-.006.049-.085.066-.135.066-.055 0-.092-.043-.141-.068-.052-.018-.146-.008-.163-.065zm-.551 0c-.02.058-.113.049-.166.066-.047.025-.086.068-.14.068-.05 0-.13-.02-.136-.068-.01-.066.088-.133.15-.133.08-.031.184-.047.259-.005.019.009.036.03.03.05v.02h.003z"/></svg><span style="display:flex;flex-direction:column;line-height:16px"><span style="font-size:11.5px;color:var(--color-muted, oklch(0.708 0 0))">Linux, x86_64 or aarch64</span><span style="display:none">: </span><span style="font-size:13px;color:var(--color-fg, oklch(0.97 0 0))">glibc ≥ 2.35</span></span></li>
<li style="display:flex;align-items:center;gap:7px;min-height:32px;box-sizing:border-box;margin:0;padding:5px 8px;border:1px solid var(--color-border-strong, oklch(0.371 0 0));border-radius:4px;background:var(--color-code-bg, oklch(0.145 0 0));color:var(--color-fg, oklch(0.97 0 0));text-decoration:none"><svg aria-hidden="true" width="16" height="16" viewBox="0 0 24 24" fill="currentColor" style="display:block;flex:none;color:var(--color-muted, oklch(0.708 0 0))"><path d="M3 15.731c1.968 1.507 5.234 2.269 9 2.269c3.76 0 7.025 -.76 9 -2.252v2.252c0 2.425 -3.895 3.936 -8.693 3.998l-.307 .002c-4.938 0 -9 -1.523 -9 -4z"/><path d="M3 9.731c1.968 1.507 5.234 2.269 9 2.269c3.76 0 7.025 -.76 9 -2.252v2.252c0 2.477 -4.062 4 -9 4c-4.798 0 -8.77 -1.438 -8.979 -3.795l-.016 -.101l-.005 -.104z"/><path d="M12 2c1.041 0 2.044 .068 2.977 .198l.469 .071q .84 .14 1.586 .348l.44 .131l.075 .024a11 11 0 0 1 .805 .3l.199 .086q .535 .242 .967 .53q .165 .11 .313 .225a3.8 3.8 0 0 1 .669 .668l.091 .128q .07 .105 .129 .211l.07 .139q .163 .35 .2 .73l.01 .211c0 2.477 -4.062 4 -9 4c-4.798 0 -8.77 -1.438 -8.979 -3.795a1 1 0 0 1 -.021 -.205l.005 -.104l.016 -.1c.205 -2.306 4.01 -3.733 8.667 -3.794z"/></svg><span style="display:flex;flex-direction:column;line-height:16px"><span style="font-size:11.5px;color:var(--color-muted, oklch(0.708 0 0))">Disk</span><span style="display:none">: </span><span style="font-size:13px;color:var(--color-fg, oklch(0.97 0 0))">~30 GB</span></span></li>
<li style="margin:0;padding:0"><a href="https://github.com/rerun-io/examples-monorepo/tree/main/packages/metrictrail" style="display:flex;align-items:center;gap:7px;min-height:32px;box-sizing:border-box;margin:0;padding:5px 8px;border:1px solid var(--color-border-strong, oklch(0.371 0 0));border-radius:4px;background:var(--color-code-bg, oklch(0.145 0 0));color:var(--color-fg, oklch(0.97 0 0));text-decoration:none"><svg aria-hidden="true" width="16" height="16" viewBox="0 0 24 24" fill="currentColor" style="display:block;flex:none;color:var(--color-muted, oklch(0.708 0 0))"><path d="M5.315 2.1c.791 -.113 1.9 .145 3.333 .966l.272 .161l.16 .1l.397 -.083a13.3 13.3 0 0 1 4.59 -.08l.456 .08l.396 .083l.161 -.1c1.385 -.84 2.487 -1.17 3.322 -1.148l.164 .008l.147 .017l.076 .014l.05 .011l.144 .047a1 1 0 0 1 .53 .514a5.2 5.2 0 0 1 .397 2.91l-.047 .267l-.046 .196l.123 .163c.574 .795 .93 1.728 1.03 2.707l.023 .295l.007 .272c0 3.855 -1.659 5.883 -4.644 6.68l-.245 .061l-.132 .029l.014 .161l.008 .157l.004 .365l-.002 .213l-.003 3.834a1 1 0 0 1 -.883 .993l-.117 .007h-6a1 1 0 0 1 -.993 -.883l-.007 -.117v-.734c-1.818 .26 -3.03 -.424 -4.11 -1.878l-.535 -.766c-.28 -.396 -.455 -.579 -.589 -.644l-.048 -.019a1 1 0 0 1 .564 -1.918c.642 .188 1.074 .568 1.57 1.239l.538 .769c.76 1.079 1.36 1.459 2.609 1.191l.001 -.678l-.018 -.168a5.03 5.03 0 0 1 -.021 -.824l.017 -.185l.019 -.12l-.108 -.024c-2.976 -.71 -4.703 -2.573 -4.875 -6.139l-.01 -.31l-.004 -.292a5.6 5.6 0 0 1 .908 -3.051l.152 -.222l.122 -.163l-.045 -.196a5.2 5.2 0 0 1 .145 -2.642l.1 -.282l.106 -.253a1 1 0 0 1 .529 -.514l.144 -.047l.154 -.03z"/></svg><span style="display:flex;flex-direction:column;line-height:16px"><span style="font-size:11.5px;color:var(--color-muted, oklch(0.708 0 0))">GitHub, examples-monorepo</span><span style="display:none">: </span><span style="font-size:13px;color:var(--color-fg, oklch(0.97 0 0));text-decoration:underline;text-decoration-color:var(--color-border-strong, oklch(0.371 0 0));text-underline-offset:3px">packages/metrictrail</span></span></a></li>
</ul>

<p style="margin:10px 0 0;font-size:13px;line-height:20px;color:var(--color-muted, oklch(0.708 0 0))">MetricTrail needs an NVIDIA GPU with more than 12 GB of memory and a driver for CUDA 13; on a 12 GB RTX 3060 the run fails in calibration at about 11.8 GB.</p>

### With a coding agent

Paste this prompt into any coding agent, or into the agent panel of the Rerun viewer.
It checks your machine, asks before it installs anything, and runs the example.

```agent-prompt
Install and run the Rerun MetricTrail example on this machine: NVIDIA's MetricTrail tracks a moving camera through one uncalibrated video and logs the camera path, the focal length, metric depth and a mesh to the Rerun viewer. Do the steps in order and stop at the first that fails.

1. Check the machine. This example needs Linux (x86_64 or aarch64) with an NVIDIA GPU that has more than 12 GB of memory and a driver for CUDA 13 or newer. Run `uname -sm`, `nvidia-smi`, `nvidia-smi --query-gpu=name,memory.total --format=csv,noheader` and `ldd --version`. If it is not Linux, `nvidia-smi` is missing, the "CUDA Version" it prints is below 13.0, the GPU's total memory is 12288 MiB or less (on a 12 GB RTX 3060 the run fails in calibration at about 11.8 GB; if it prints `[N/A]`, as a DGX Spark does with its unified memory, use the total memory from `free -g` instead), or glibc is older than 2.35, stop: tell me why, and that I can try the example in the browser at https://pablovela5620-metrictrail-rerun.hf.space instead.
2. Check Pixi with `pixi --version`; it must be 0.83 or newer. If `pixi` is not found, try `~/.pixi/bin/pixi --version` before you offer to install it; if that works, put `~/.pixi/bin` on PATH. Use Pixi for everything; do not use pip, uv or conda.
3. Choose where the code goes. Propose `~/rerun-examples/examples-monorepo` (not /tmp: the clone with its environment takes about 19 GB, the model caches about 5 GB more, and both must survive a reboot; on a clean machine Pixi's package cache also grows by the downloaded packages, so count that space too). Run `df -h` on its nearest existing parent (e.g. `df -h ~`), if the folder does not exist yet. Then ask me ONE question that covers everything: the folder, the free space there, and, if needed, that you will install Pixi (`curl -fsSL https://pixi.sh/install.sh | sh`) or update it (`pixi self-update`). Wait for my answer, and use another folder if I name one. If less than 30 GB is free there, say so in the question.
4. Get the code. If the folder already holds a clone of https://github.com/rerun-io/examples-monorepo, run `git pull` in it; otherwise `git clone --depth 1 https://github.com/rerun-io/examples-monorepo.git <folder>`. Then `cd <folder>`. If you installed Pixi, open a new shell (or source the shell rc) so `pixi` is on PATH.
5. Pick where the output goes, then run. Base command: `pixi run --frozen -e metrictrail metrictrail-example <output flags>`. The first run prints about 3,000 lines: send them to a log file (`<command> > run.log 2>&1`, which keeps the exit code) and search that file. It tracks the default clip `IMG_0497.mp4`, a 1:58 walk along a stone balustrade (every third frame, 1,177 frames), and writes its outputs to `packages/metrictrail/data/example/balustrade`. The video on the Rerun example page comes from another clip, 23 s of `pool.mp4` (see step 7). The first run installs a large CUDA environment, builds three Mojo kernel libraries, downloads the weights (about 120 MB: two files from Hugging Face and droid.pth from Google Drive, plus the 1.3 GB MoGe-2 checkpoint from Hugging Face) and six example clips (437 MB), and builds TensorRT engines. On a DGX Spark that already had the conda packages, MoGe-2 and droid.pth cached, a first run from a fresh clone took about 6 minutes; on a clean machine these downloads add to that. Later runs take about 3 minutes 20 seconds on a DGX Spark. If the first run stops with `Segmentation fault` (exit code 139) while it builds the TensorRT engines, run the same command once more: this happened once on a DGX Spark, and the second try worked. A new run overwrites the outputs and the .rrd file.
   - If you have Rerun viewer tools (you run inside the Rerun viewer's agent panel): send the data to that viewer. Call `rerun_connect` with no endpoint; its reply names the viewer's address, e.g. `http://127.0.0.1:41881`. Use `--rr-config.connect --rr-config.port <that port>`. The video, depth, calibration, camera path and mesh then appear in the viewer you are in.
   - Otherwise, with a display: no output flags. The command spawns a Rerun viewer on port 9876. If a Rerun viewer already listens there, the data shows in that viewer. If something else listens there (`ss -ltnp | grep ':9876 '`), for example a `rerun server`, it swallows the data and you see nothing: then add `--rr-config.port <a free port>`.
   - Otherwise, with no display (for example over SSH, when `DISPLAY` and `WAYLAND_DISPLAY` are both empty): `--rr-config.save <folder>/metrictrail.rrd` with an absolute path (the task runs in `packages/metrictrail`, so a relative path lands there; the file is about 385 MB), and tell me to open that file with the Rerun viewer on a machine with a screen (`pixi run --frozen -e metrictrail rerun <folder>/metrictrail.rrd`, or https://rerun.io/viewer).
6. Check the run. Build warnings (setuptools, Mojo, `[TRT] [W] WARNING The logger passed into`) and `Terminating` are normal. The run succeeded if the command exits 0 and its last line starts with `RunMetrics(num_tracked=1177, num_finite=1177`. With `--rr-config.save`, also check that the .rrd file exists and is about 385 MB. The output must not contain `Failed to flush sink` or `connection severed`. If it does, the port belongs to something that is not a viewer; fix the port as in step 5 and run again. Inside the viewer's agent panel, also call `rerun_get_state` and check that a recording with the application id `run` and the timelines `video_time` and `calibration_step` is open.
7. When it finishes, tell me what ran, how long it took, the folder and its size on disk (`du -sh <folder>`), and that deleting that folder removes the code, its environment, the weights, the clips and the run outputs. Say that some caches stay, where other projects can reuse them: the downloaded packages in Pixi's cache, MoGe-2 in `~/.cache/huggingface`, the TensorRT engines in `~/.cache/monoprior` and `~/.cache/trtkit`, and `droid.pth` in `~/.cache/torch`. Tell me that I can run my own clip with `pixi run --frozen -e metrictrail metrictrail-run --input <absolute path to my video.mp4> --benchmark video --output <absolute output folder> <output flags>`. Tell me that the video on the Rerun example page comes from another clip, 23 s of `pool.mp4`: `pixi run --frozen -e metrictrail _download-metrictrail-iphone-data` puts it in `packages/metrictrail/data/iphone/pool.mp4`, and the "Manually" section of that page shows how to cut those 23 s and run them.
```

### Manually

Install [Pixi](https://pixi.sh/latest/#installation) 0.83 or newer, then:

```sh
git clone --depth 1 https://github.com/rerun-io/examples-monorepo.git
cd examples-monorepo
pixi run --frozen -e metrictrail metrictrail-example
```

This runs the default clip, `IMG_0497.mp4`: a 1:58 walk along a stone balustrade. Outputs go to `data/example/balustrade` in `packages/metrictrail`.
The first run installs the environment, builds the Mojo kernels and TensorRT engines, and downloads the weights and six example clips; with the conda packages, MoGe-2 and droid.pth already cached, it took about 6 minutes on a DGX Spark.
Without a display, add `--rr-config.save /abs/path/balustrade.rrd`.

The video and pictures on this page come from another clip: 23 s (0:44 to 1:07) of `pool.mp4`; the video at the top shows the first 17 s of that run. These commands, run from the `examples-monorepo` folder, download it, cut the same 23 s and track them:

```sh
pixi run --frozen -e metrictrail _download-metrictrail-iphone-data
.pixi/envs/metrictrail/bin/ffmpeg -ss 44.0 -i packages/metrictrail/data/iphone/pool.mp4 -t 23.0 \
  -vf scale=1280:720:flags=lanczos -c:v libx264 -preset slow -crf 26 -maxrate 3M -bufsize 6M \
  -bf 0 -g 30 -keyint_min 30 -sc_threshold 0 -pix_fmt yuv420p -an -movflags +faststart pool-rooftop-23s.mp4
pixi run --frozen -e metrictrail metrictrail-run --input $PWD/pool-rooftop-23s.mp4 --benchmark video --output $PWD/pool-out
```

On a DGX Spark, this tracked all 690 frames.

To run your own video, use `pixi run --frozen -e metrictrail metrictrail-run --input /abs/path/video.mp4 --benchmark video --output /abs/path/out` with absolute paths (the task runs in `packages/metrictrail`).

## The result

The video at the top shows the first 17 s of a 23 s phone video on a rooftop terrace, with no calibration given.
Left, top: the video. The amber boxes are the 64 patches of the newest keyframe: small image squares that the DPVO tracker follows from frame to frame.
Left, bottom: the predicted metric depth, 0.5 m (blue) to 20 m (red).
Right: the final camera path and a mesh of the terrace, with the camera moving along the path.
MetricTrail tracks all 690 frames, finds the focal length from the clip itself, and fuses the depth of every frame into one mesh of 1.18 million triangles.
This run uses the code of pull requests #579 and #582 to [#585](https://github.com/rerun-io/examples-monorepo/pull/585), which are merged into `main`.
A run of the same 23 s at commit 4e0ff2d9e (`main` after pull request #596) gives numbers close to these, but not equal: for example fx = 377.97 px instead of 378.60 px (step 2).

The same 690 frames on an NVIDIA RTX 5090, from the video file to the mesh (timed on a quiet machine at commit 8148306a5; the pictures come from commit 8a77f49cf, which changes only how patches are drawn):

| Pass | Total | Calibration | Tracking | Speed |
|---|---|---|---|---|
| With capture | 39.95 s | 9.90 s | 25.48 s | 27.1 fps |
| No capture | 27.84 s | 10.18 s | 10.95 s | 63.0 fps |

Mesh fusion takes 2.30 s and 1.98 s.

Where the time goes, per stage: in each pair, the left bar is with capture and the right bar is without. Without capture, the run logs the saved depth maps after tracking, which takes 2.7 s. Other pipeline work is model loading and setup.

<picture>
  <img src="https://static.rerun.io/metrictrail_seg_stages_v5/e2c397989d3512fbe485fb638a98a14d94a08a88/full.webp" alt="Runtime per stage of this run as one Rerun bar chart, with and without capture">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/metrictrail_seg_stages_v5/e2c397989d3512fbe485fb638a98a14d94a08a88/480w.webp">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/metrictrail_seg_stages_v5/e2c397989d3512fbe485fb638a98a14d94a08a88/768w.webp">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/metrictrail_seg_stages_v5/e2c397989d3512fbe485fb638a98a14d94a08a88/1024w.webp">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/metrictrail_seg_stages_v5/e2c397989d3512fbe485fb638a98a14d94a08a88/1200w.webp">
</picture>

## How one frame goes through it

Frame 345 of the run above, 11.5 s into the clip: the camera walks along the pool, with the deck on the left and the city behind the glass railing.

<picture>
  <img src="https://static.rerun.io/metrictrail_metrictrail_flow_dag_v3/c168b0b16ddef89cb4d64167a0f71a4dc10d2e61/full.webp" alt="The pipeline on frame 345 as a graph: the input frame feeds calibration, image features and metric depth; calibration gives the focal length to metric depth; features and depth feed the patches; patches carry MoGe-2's depth as their prior into bundle adjustment, which solves 10 cameras and 1,408 patch depths and gives the poses of the output; the output mesh also fuses the depth">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/metrictrail_metrictrail_flow_dag_v3/c168b0b16ddef89cb4d64167a0f71a4dc10d2e61/480w.webp">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/metrictrail_metrictrail_flow_dag_v3/c168b0b16ddef89cb4d64167a0f71a4dc10d2e61/768w.webp">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/metrictrail_metrictrail_flow_dag_v3/c168b0b16ddef89cb4d64167a0f71a4dc10d2e61/1024w.webp">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/metrictrail_metrictrail_flow_dag_v3/c168b0b16ddef89cb4d64167a0f71a4dc10d2e61/1200w.webp">
</picture>

### 1. The input frame

Frame 345 is one frame of the 23 s cut, scaled from the 1920 × 1080 phone video to 1280 × 720, with no calibration given; the tracker reads a copy scaled to 504 × 280. Right: the camera of this frame (white) at its final pose on the path, over the mesh, with a grid of 1 m squares on the deck.

<picture>
  <img src="https://static.rerun.io/metrictrail_walk_1_frame_v4/74189fd8a873b399c72243cbac48673cc5a50e40/full.webp" alt="Frame 345 next to its camera on the final path over the mesh, with a 1 m grid on the deck">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/metrictrail_walk_1_frame_v4/74189fd8a873b399c72243cbac48673cc5a50e40/480w.webp">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/metrictrail_walk_1_frame_v4/74189fd8a873b399c72243cbac48673cc5a50e40/768w.webp">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/metrictrail_walk_1_frame_v4/74189fd8a873b399c72243cbac48673cc5a50e40/1024w.webp">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/metrictrail_walk_1_frame_v4/74189fd8a873b399c72243cbac48673cc5a50e40/1200w.webp">
</picture>

### 2. One focal length for the clip

MoGe-2's depth and every camera pose need the focal length that step 1 lacks, so before tracking SharedK fits one camera matrix for the whole clip. MoGe-2 guesses fx and fy on each of the first 32 frames (gray dots: fx, 355.2 to 395.0 px), and SharedK starts from their medians, fx = 375.75 px and fy = 369.33 px. A DROID-style keyframe SLAM on 43 calibration keyframes (left) then moves fx and fy by one shared offset: first down to fx = 374.08 px, then up to fx = 378.60 px (cyan dots: three solves near step 165, two final solves at step 689). The x axis is the calibration step, one per frame of the clip.

The pixels are those of the 504 × 280 tracking image: on the 1280 px frame, fx is about 961.5 px.

<picture>
  <img src="https://static.rerun.io/metrictrail_walk_2_calibration_v4/042c7a288e23be92ae04591f9c8a6cea71313215/full.webp" alt="The 43-keyframe calibration map next to the focal length after each solve, over the calibration steps">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/metrictrail_walk_2_calibration_v4/042c7a288e23be92ae04591f9c8a6cea71313215/480w.webp">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/metrictrail_walk_2_calibration_v4/042c7a288e23be92ae04591f9c8a6cea71313215/768w.webp">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/metrictrail_walk_2_calibration_v4/042c7a288e23be92ae04591f9c8a6cea71313215/1024w.webp">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/metrictrail_walk_2_calibration_v4/042c7a288e23be92ae04591f9c8a6cea71313215/1200w.webp">
</picture>

### 3. Image features

[DINOv3](https://arxiv.org/abs/2508.10104) turns the frame into an 18 × 32 grid of feature vectors (the flow picture above shows them). A small trained adapter adds them to the features that DPVO uses to match patches (step 5). DPVO's own features describe small local texture, and DINOv3 describes larger regions. The adapter starts from zero in training, so it begins as plain DPVO and learns only what DINOv3 adds.

From left, the picture shows frame 345, DPVO's own features, the DINOv3 term, and their sum. Colors show the three strongest directions of each feature map (PCA) as RGB. DPVO's own features and the sum use the same PCA. Per cell, the DINOv3 term is a median 9 % of the length of DPVO's own feature vector, so the two look almost the same.

<picture>
  <img src="https://static.rerun.io/metrictrail_walk_3_fusion_v3/d040e56ac34264da64340e8290ade1652f16aa0d/full.webp" alt="Frame 345, DPVO's own matching features, the DINOv3 adapter term and their sum, as PCA colors">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/metrictrail_walk_3_fusion_v3/d040e56ac34264da64340e8290ade1652f16aa0d/480w.webp">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/metrictrail_walk_3_fusion_v3/d040e56ac34264da64340e8290ade1652f16aa0d/768w.webp">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/metrictrail_walk_3_fusion_v3/d040e56ac34264da64340e8290ade1652f16aa0d/1024w.webp">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/metrictrail_walk_3_fusion_v3/d040e56ac34264da64340e8290ade1652f16aa0d/1200w.webp">
</picture>

### 4. Metric depth and normals

[MoGe-2](https://arxiv.org/abs/2507.02546) gives every pixel a depth in meters. Later steps use it: DPVO starts its patches at this depth, and bundle adjustment pulls the path toward it, which fixes the scale of the path.

MoGe-2 has its own encoder, a DINOv2 ViT-L, separate from the DINOv3 of step 3. The encoder turns the frame, resized to 644 × 364, into a 26 × 46 grid of tokens. Image-filter (conv) heads read the tokens and predict a 3D point map with no unit and a mask that drops pixels with no usable depth (0.01 % here). A small MLP reads one summary vector of the whole image (the CLS token) and predicts one scale, s = 5.19. MoGe-2 adds a small shift (-0.05) to z: the offset with which the points, projected through the given focal length, land back on their own pixels. Then it multiplies them by s: the result is points in meters.

<picture>
  <img src="https://static.rerun.io/metrictrail_readme_walk_4_flow_v3/ee712eb20dc3ad6c5fb79440ec9b9dbd12ef8566/full.webp" alt="Inside MoGe-2: frame, DINOv2 encoder, point map, mask and scale heads, metric points, metric depth">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/metrictrail_readme_walk_4_flow_v3/ee712eb20dc3ad6c5fb79440ec9b9dbd12ef8566/480w.webp">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/metrictrail_readme_walk_4_flow_v3/ee712eb20dc3ad6c5fb79440ec9b9dbd12ef8566/768w.webp">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/metrictrail_readme_walk_4_flow_v3/ee712eb20dc3ad6c5fb79440ec9b9dbd12ef8566/1024w.webp">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/metrictrail_readme_walk_4_flow_v3/ee712eb20dc3ad6c5fb79440ec9b9dbd12ef8566/1200w.webp">
</picture>

<picture>
  <img src="https://static.rerun.io/metrictrail_readme_walk_4_inside_v2/a98e9f513c350f94075c05f138342b68f95c8cd7/full.webp" alt="Frame 345's DINOv2 tokens, the point map's z with no unit (0.1 purple to 3.9 yellow), and the metric points with the camera and a 1 m grid">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/metrictrail_readme_walk_4_inside_v2/a98e9f513c350f94075c05f138342b68f95c8cd7/480w.webp">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/metrictrail_readme_walk_4_inside_v2/a98e9f513c350f94075c05f138342b68f95c8cd7/768w.webp">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/metrictrail_readme_walk_4_inside_v2/a98e9f513c350f94075c05f138342b68f95c8cd7/1024w.webp">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/metrictrail_readme_walk_4_inside_v2/a98e9f513c350f94075c05f138342b68f95c8cd7/1200w.webp">
</picture>

The result for frame 345, with depth from 0.5 m (blue) to 20 m (red): 2.6 m to the nearest point and 6.6 m median. On this frame alone, MoGe-2 guesses a focal length of 413.4 px, and its guesses change from frame to frame (step 2). So the example gives it the one SharedK focal length, 378.60 px on the 504 × 280 tracking image. With its own guess, the median depth would be 7.0 m.

The same pass also gives surface normals (color = direction: the deck, which faces up, is magenta). The tracker does not use them; the recording keeps them for display.

<picture>
  <img src="https://static.rerun.io/metrictrail_readme_walk_4_output_v2/572c17a22a770d1111084d115ce10b02e8e168c2/full.webp" alt="Frame 345, its metric depth (0.5 m blue to 20 m red), its surface normals, and the depth key">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/metrictrail_readme_walk_4_output_v2/572c17a22a770d1111084d115ce10b02e8e168c2/480w.webp">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/metrictrail_readme_walk_4_output_v2/572c17a22a770d1111084d115ce10b02e8e168c2/768w.webp">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/metrictrail_readme_walk_4_output_v2/572c17a22a770d1111084d115ce10b02e8e168c2/1024w.webp">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/metrictrail_readme_walk_4_output_v2/572c17a22a770d1111084d115ce10b02e8e168c2/1200w.webp">
</picture>

### 5. Patches start at the predicted depth

DPVO follows the frame through 64 patches. A patch is a 12 x 12 px square, 3 x 3 cells of DPVO's 4 px feature grid, on the 504 x 280 tracking frame. Each patch starts at MoGe-2's depth at its center: all 64 get a valid depth, with a median of 8.0 m (7.9 m after the frame's first bundle adjustment). DPVO draws the 64 centers at random, so their median need not match the 6.6 m median of all pixels. Plain DPVO has no metric depth: it gives all new patches one shared value, the median depth of the patches of the last three keyframes. Below, each box is colored by the depth it starts at, on the same scale as the depth map: 0.5 m (blue) to 20 m (red).

<picture>
  <img src="https://static.rerun.io/metrictrail_walk_5_patches_v2/cbd50fb8cecba97b9e896a30b1dab7ed9ffb6e4f/full.webp" alt="Left: the 64 patches colored by their start depth. Right: MoGe-2 depth with the same boxes. Both use 0.5 m (blue) to 20 m (red)">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/metrictrail_walk_5_patches_v2/cbd50fb8cecba97b9e896a30b1dab7ed9ffb6e4f/480w.webp">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/metrictrail_walk_5_patches_v2/cbd50fb8cecba97b9e896a30b1dab7ed9ffb6e4f/768w.webp">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/metrictrail_walk_5_patches_v2/cbd50fb8cecba97b9e896a30b1dab7ed9ffb6e4f/1024w.webp">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/metrictrail_walk_5_patches_v2/cbd50fb8cecba97b9e896a30b1dab7ed9ffb6e4f/1200w.webp">
</picture>

The start depth and the camera pose put each patch into the other keyframes. DPVO compares the patch's features only in a 7 x 7-cell window around that spot, so a good start depth starts the search near the answer. The recurrent update then predicts where the patch really lies (its target) and how much to trust that match (a weight from 0 to 1). The update runs again on every new frame. For the 64 patches matched into the older keyframe 342, the median weight rises from 0.10 after the first update to 0.75 after the third, two frames later. Below, purple boxes have a low weight and green to yellow boxes a high one.

<picture>
  <img src="https://static.rerun.io/metrictrail_walk_5_tracking_v3/88c1e031dafcab0a8b9e5785142575a4559d9660/full.webp" alt="Eight patches of frame 345 and their targets in frame 342 after update 1 and update 3, colored by weight from 0 (purple) to 1 (yellow)">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/metrictrail_walk_5_tracking_v3/88c1e031dafcab0a8b9e5785142575a4559d9660/480w.webp">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/metrictrail_walk_5_tracking_v3/88c1e031dafcab0a8b9e5785142575a4559d9660/768w.webp">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/metrictrail_walk_5_tracking_v3/88c1e031dafcab0a8b9e5785142575a4559d9660/1024w.webp">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/metrictrail_walk_5_tracking_v3/88c1e031dafcab0a8b9e5785142575a4559d9660/1200w.webp">
</picture>

### 6. Bundle adjustment in meters

At frame 345, DPVO's graph holds 22 keyframes: the ones whose patches still have matches. Bundle adjustment moves the newest 10 cameras and the depths of all 1,408 patches in the graph, so that each patch lands closer to the target that DPVO's network set in step 5. The 12 older cameras stay fixed as anchors. The metric depth term adds one pull per patch toward MoGe-2's depth, and that keeps the scale in meters.

<picture>
  <img src="https://static.rerun.io/metrictrail_walk_6_window_v2/c5b05f1f248524b4f6cfc6fea92a46b8e50a10c4/full.webp" alt="The bundle adjustment at frame 345 from above and from the side: window keyframes 1 to 10 are solved, 12 older keyframes stay fixed, on a 1 m grid">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/metrictrail_walk_6_window_v2/c5b05f1f248524b4f6cfc6fea92a46b8e50a10c4/480w.webp">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/metrictrail_walk_6_window_v2/c5b05f1f248524b4f6cfc6fea92a46b8e50a10c4/768w.webp">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/metrictrail_walk_6_window_v2/c5b05f1f248524b4f6cfc6fea92a46b8e50a10c4/1024w.webp">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/metrictrail_walk_6_window_v2/c5b05f1f248524b4f6cfc6fea92a46b8e50a10c4/1200w.webp">
</picture>

Amber frustums (camera pyramids) 1 to 10 are solved, gray ones stay fixed, and frame 345 is white. Keyframes 7 to 10 are frames 342 to 345: DPVO adds every frame as a keyframe and later drops the ones that add little motion. Blue points belong to the 640 patches of the window keyframes, green points to the 768 patches of the older keyframes. Bundle adjustment solves the depths of both.

<picture>
  <img src="https://static.rerun.io/metrictrail_walk_6_prior_v2/128b2f3e4e13570f6091bbc7abfacba1a3a3b7c3/full.webp" alt="Window patches at frame 345, MoGe-2 depth against bundle adjustment depth: with the metric depth term the points follow the equal line; with its weight at 0 they fall below it">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/metrictrail_walk_6_prior_v2/128b2f3e4e13570f6091bbc7abfacba1a3a3b7c3/480w.webp">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/metrictrail_walk_6_prior_v2/128b2f3e4e13570f6091bbc7abfacba1a3a3b7c3/768w.webp">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/metrictrail_walk_6_prior_v2/128b2f3e4e13570f6091bbc7abfacba1a3a3b7c3/1024w.webp">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/metrictrail_walk_6_prior_v2/128b2f3e4e13570f6091bbc7abfacba1a3a3b7c3/1200w.webp">
</picture>

Each point above is one window patch at frame 345: MoGe-2's depth across, the bundle adjustment depth up. With the term, the median gap is 5.5 % (0.42 m). Track the whole clip with the term's weight at 0, and most depths shrink, while some far ones drift past 20 m onto the edge: the median gap grows to 33 % (2.1 m). The reprojection error (how far a patch lands from its target) stays almost the same, 1.04 px with the term and 1.06 px without, so the matches alone cannot fix the scale.

<picture>
  <img src="https://static.rerun.io/metrictrail_walk_6_noprior_v2/c384e69285d5fe6890fb4a247db0026423c73973/full.webp" alt="The final keyframe path from above, with the metric depth term (amber, 28.8 m) and with its weight at 0 (purple, 24.5 DPVO units)">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/metrictrail_walk_6_noprior_v2/c384e69285d5fe6890fb4a247db0026423c73973/480w.webp">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/metrictrail_walk_6_noprior_v2/c384e69285d5fe6890fb4a247db0026423c73973/768w.webp">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/metrictrail_walk_6_noprior_v2/c384e69285d5fe6890fb4a247db0026423c73973/1024w.webp">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/metrictrail_walk_6_noprior_v2/c384e69285d5fe6890fb4a247db0026423c73973/1200w.webp">
</picture>

The path shrinks too: 24.5 DPVO units against 28.8 m with the term, 15 % shorter. The clip has no ground truth, so this shows scale drift, not which length is right. MetricTrail rejects a weight-0 run at its end; this path comes from the tracker before that check.

### 7. Output

The run returns 690 camera poses in meters and a mesh of 1,181,157 triangles, fused from MoGe-2's depth at the final poses (the fusion keeps depths from 0.2 to 8 m, so the far city is not in the mesh). A known size checks the scale: the camera rides about 1.5 m above the deck (1.2 to 1.7 m along the path), the height at which a person holds a phone. The grid squares are 1 m and help to read such sizes. Right: the same mesh colored by its normals, so the flat deck and the walls stand out.

<picture>
  <img src="https://static.rerun.io/metrictrail_walk_7_output_v4/54c87f03622057b3561e81f025db67c3ea13d516/full.webp" alt="The mesh with the final path, the camera of frame 345 and a 1 m grid, and the same mesh colored by its normals">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/metrictrail_walk_7_output_v4/54c87f03622057b3561e81f025db67c3ea13d516/480w.webp">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/metrictrail_walk_7_output_v4/54c87f03622057b3561e81f025db67c3ea13d516/768w.webp">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/metrictrail_walk_7_output_v4/54c87f03622057b3561e81f025db67c3ea13d516/1024w.webp">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/metrictrail_walk_7_output_v4/54c87f03622057b3561e81f025db67c3ea13d516/1200w.webp">
</picture>

## How it works

MetricTrail runs these stages on the video:

1. **Calibration (once, before tracking).** SharedK finds one focal length for the whole clip. MoGe-2 gives a first guess on 32 frames, then a DROID-style keyframe SLAM, separate from the tracker, refines it (43 keyframes on this clip). The tracker uses this focal length on every frame.
2. **Features and depth (every frame).** A frozen DINOv3 model and MoGe-2 both read the frame; neither needs the other. A small trained adapter adds the DINOv3 features to the features that DPVO uses to match patches. MoGe-2 predicts metric depth and surface normals in one TensorRT pass.
3. **Patches.** DPVO samples 64 patches per frame. Each patch starts at MoGe-2's depth at that pixel, not at one shared guess for all patches (plain DPVO uses the median depth of the patches of the last three keyframes), and keeps that depth as its prior.
4. **Bundle adjustment.** A recurrent update matches each patch in recent keyframes and gives a target and a weight. Over a small window of recent keyframes, bundle adjustment adds a soft term that pulls each patch depth toward its prior. This keeps the camera path in meters.
5. **Surface.** This example fuses the depth of every tracked frame at its final pose into a TSDF with 4 cm voxels (in Rust), and marching cubes turns it into one mesh.

For Rerun, the example logs the input video file as a video stream, so the recording keeps the original video samples instead of one image per frame.
It logs the calibration step by step on its own `calibration_step` timeline: the keyframe map, the depth and the focal length as it converges.
One default layout shows the video, depth, normals and features on the left, and tabs for calibration, tracking and the final surface on the right.

## Papers and credits

* **MetricTrail**: [paper](https://research.nvidia.com/labs/sil/projects/metrictrail/assets/MetricTrail.pdf) and [project page](https://research.nvidia.com/labs/sil/projects/metrictrail/), Gao et al. (NVIDIA, 2026), with [code](https://github.com/nv-tlabs/vipe) and [weights](https://huggingface.co/nvidia/MetricTrail).
* [ViPE: Video Pose Engine for 3D Geometric Perception](https://arxiv.org/abs/2508.10934), Huang et al. (NVIDIA, 2025), with [code](https://github.com/nv-tlabs/vipe).
* [Deep Patch Visual Odometry](https://arxiv.org/abs/2208.04726) (DPVO), Teed, Lipson and Deng (NeurIPS 2023), with [code](https://github.com/princeton-vl/DPVO).
* [MoGe-2: Accurate Monocular Geometry with Metric Scale and Sharp Details](https://arxiv.org/abs/2507.02546), Wang et al. (NeurIPS 2025), with [code](https://github.com/microsoft/MoGe) and [weights](https://huggingface.co/Ruicheng/moge-2-vitl-normal).
* [DINOv3](https://arxiv.org/abs/2508.10104), Siméoni et al. (Meta, 2025), with [code](https://github.com/facebookresearch/dinov3) and [weights](https://huggingface.co/facebook/dinov3-vits16-pretrain-lvd1689m).
* [DROID-SLAM: Deep Visual SLAM for Monocular, Stereo, and RGB-D Cameras](https://arxiv.org/abs/2108.10869), Teed and Deng (NeurIPS 2021), with [code](https://github.com/princeton-vl/DROID-SLAM). SharedK calibration uses it.
* TSDF fusion: [A Volumetric Method for Building Complex Models from Range Images](https://graphics.stanford.edu/papers/volrange/), Curless and Levoy (SIGGRAPH 1996). Truncation and voxel conventions from [Open3D](https://github.com/isl-org/Open3D) ([paper](https://arxiv.org/abs/1801.09847)); marching-cubes table from [Paul Bourke](https://paulbourke.net/geometry/polygonise/).
