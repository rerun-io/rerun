<!--[metadata]
title = "EgoHand: 3D hands from egocentric video"
description = "Find both hands in a head-camera video and turn each one into metric 3D joints and a SOMA hand mesh with NVIDIA's EgoHand, step by step."
tags = ["2D", "3D", "Egocentric", "Human mesh", "Keypoint detection", "Hugging Face"]
source = "https://github.com/rerun-io/examples-monorepo/tree/main/packages/posekit"
thumbnail = "https://static.rerun.io/egohand_thumbnail_768x576/a8e0f1124229fe08965ddd318366b47abae2fb56/full.png"
thumbnail_dimensions = [768, 576]
preview_rrd_url = "https://huggingface.co/datasets/pablovela5620/egohand-rerun-example/resolve/main/egohand-gen-humanego-segment.rrd"
-->

[![Open in Hugging Face Spaces](https://huggingface.co/datasets/huggingface/badges/resolve/main/open-in-hf-spaces-md.svg)](https://pablovela5620-egohand-rerun.hf.space)

```agent-prompt
```

Try it in your browser with no install (the Space runs the fast path on ZeroGPU), or copy a prompt for your coding agent to install and run it on your own Linux machine with an NVIDIA GPU.

<video src="https://static.rerun.io/b878cfc3171606bada00d086160db9511731e6d2_seg-run.mp4" poster="https://static.rerun.io/d64429345c4839f7fdcd009a0a43c709d2198064_seg-run.webp" autoplay muted loop playsinline></video>

EgoHand finds both hands in each frame of a head-worn camera and turns each hand into a metric 3D mesh.
This example runs [NVIDIA's EgoHand](https://huggingface.co/nvidia/EgoHand-v1) on a head-camera video and logs every step to Rerun: the video with hand boxes and 2D joints, and a 3D view with the camera, the 3D joints and a SOMA hand mesh per hand.

## Run the code

<ul aria-label="Requirements" style="display:flex;flex-wrap:wrap;gap:6px;list-style:none;margin:20px 0 0;padding:0">
<li style="display:flex;align-items:center;gap:7px;min-height:32px;box-sizing:border-box;margin:0;padding:5px 8px;border:1px solid var(--color-border-strong, oklch(0.371 0 0));border-radius:4px;background:var(--color-code-bg, oklch(0.145 0 0));color:var(--color-fg, oklch(0.97 0 0));text-decoration:none"><svg aria-hidden="true" width="16" height="16" viewBox="0 0 24 24" fill="currentColor" style="display:block;flex:none;color:var(--color-muted, oklch(0.708 0 0))"><path d="M8.948 8.798v-1.43a6.7 6.7 0 0 1 .424-.018c3.922-.124 6.493 3.374 6.493 3.374s-2.774 3.851-5.75 3.851c-.398 0-.787-.062-1.158-.185v-4.346c1.528.185 1.837.857 2.747 2.385l2.04-1.714s-1.492-1.952-4-1.952a6.016 6.016 0 0 0-.796.035m0-4.735v2.138l.424-.027c5.45-.185 9.01 4.47 9.01 4.47s-4.08 4.964-8.33 4.964c-.37 0-.733-.035-1.095-.097v1.325c.3.035.61.062.91.062 3.957 0 6.82-2.023 9.593-4.408.459.371 2.34 1.263 2.73 1.652-2.633 2.208-8.772 3.984-12.253 3.984-.335 0-.653-.018-.971-.053v1.864H24V4.063zm0 10.326v1.131c-3.657-.654-4.673-4.46-4.673-4.46s1.758-1.944 4.673-2.262v1.237H8.94c-1.528-.186-2.73 1.245-2.73 1.245s.68 2.412 2.739 3.11M2.456 10.9s2.164-3.197 6.5-3.533V6.201C4.153 6.59 0 10.653 0 10.653s2.35 6.802 8.948 7.42v-1.237c-4.84-.6-6.492-5.936-6.492-5.936z"/></svg><span style="display:flex;flex-direction:column;line-height:16px"><span style="font-size:11.5px;color:var(--color-muted, oklch(0.708 0 0))">NVIDIA GPU</span><span style="display:none">: </span><span style="font-size:13px;color:var(--color-fg, oklch(0.97 0 0))">CUDA 13 driver</span></span></li>
<li style="display:flex;align-items:center;gap:7px;min-height:32px;box-sizing:border-box;margin:0;padding:5px 8px;border:1px solid var(--color-border-strong, oklch(0.371 0 0));border-radius:4px;background:var(--color-code-bg, oklch(0.145 0 0));color:var(--color-fg, oklch(0.97 0 0));text-decoration:none"><svg aria-hidden="true" width="16" height="16" viewBox="0 0 24 24" fill="currentColor" style="display:block;flex:none;color:var(--color-muted, oklch(0.708 0 0))"><path d="M12.504 0c-.155 0-.315.008-.48.021-4.226.333-3.105 4.807-3.17 6.298-.076 1.092-.3 1.953-1.05 3.02-.885 1.051-2.127 2.75-2.716 4.521-.278.832-.41 1.684-.287 2.489a.424.424 0 00-.11.135c-.26.268-.45.6-.663.839-.199.199-.485.267-.797.4-.313.136-.658.269-.864.68-.09.189-.136.394-.132.602 0 .199.027.4.055.536.058.399.116.728.04.97-.249.68-.28 1.145-.106 1.484.174.334.535.47.94.601.81.2 1.91.135 2.774.6.926.466 1.866.67 2.616.47.526-.116.97-.464 1.208-.946.587-.003 1.23-.269 2.26-.334.699-.058 1.574.267 2.577.2.025.134.063.198.114.333l.003.003c.391.778 1.113 1.132 1.884 1.071.771-.06 1.592-.536 2.257-1.306.631-.765 1.683-1.084 2.378-1.503.348-.199.629-.469.649-.853.023-.4-.2-.811-.714-1.376v-.097l-.003-.003c-.17-.2-.25-.535-.338-.926-.085-.401-.182-.786-.492-1.046h-.003c-.059-.054-.123-.067-.188-.135a.357.357 0 00-.19-.064c.431-1.278.264-2.55-.173-3.694-.533-1.41-1.465-2.638-2.175-3.483-.796-1.005-1.576-1.957-1.56-3.368.026-2.152.236-6.133-3.544-6.139zm.529 3.405h.013c.213 0 .396.062.584.198.19.135.33.332.438.533.105.259.158.459.166.724 0-.02.006-.04.006-.06v.105a.086.086 0 01-.004-.021l-.004-.024a1.807 1.807 0 01-.15.706.953.953 0 01-.213.335.71.71 0 00-.088-.042c-.104-.045-.198-.064-.284-.133a1.312 1.312 0 00-.22-.066c.05-.06.146-.133.183-.198.053-.128.082-.264.088-.402v-.02a1.21 1.21 0 00-.061-.4c-.045-.134-.101-.2-.183-.333-.084-.066-.167-.132-.267-.132h-.016c-.093 0-.176.03-.262.132a.8.8 0 00-.205.334 1.18 1.18 0 00-.09.4v.019c.002.089.008.179.02.267-.193-.067-.438-.135-.607-.202a1.635 1.635 0 01-.018-.2v-.02a1.772 1.772 0 01.15-.768c.082-.22.232-.406.43-.533a.985.985 0 01.594-.2zm-2.962.059h.036c.142 0 .27.048.399.135.146.129.264.288.344.465.09.199.14.4.153.667v.004c.007.134.006.2-.002.266v.08c-.03.007-.056.018-.083.024-.152.055-.274.135-.393.2.012-.09.013-.18.003-.267v-.015c-.012-.133-.04-.2-.082-.333a.613.613 0 00-.166-.267.248.248 0 00-.183-.064h-.021c-.071.006-.13.04-.186.132a.552.552 0 00-.12.27.944.944 0 00-.023.33v.015c.012.135.037.2.08.334.046.134.098.2.166.268.01.009.02.018.034.024-.07.057-.117.07-.176.136a.304.304 0 01-.131.068 2.62 2.62 0 01-.275-.402 1.772 1.772 0 01-.155-.667 1.759 1.759 0 01.08-.668 1.43 1.43 0 01.283-.535c.128-.133.26-.2.418-.2zm1.37 1.706c.332 0 .733.065 1.216.399.293.2.523.269 1.052.468h.003c.255.136.405.266.478.399v-.131a.571.571 0 01.016.47c-.123.31-.516.643-1.063.842v.002c-.268.135-.501.333-.775.465-.276.135-.588.292-1.012.267a1.139 1.139 0 01-.448-.067 3.566 3.566 0 01-.322-.198c-.195-.135-.363-.332-.612-.465v-.005h-.005c-.4-.246-.616-.512-.686-.71-.07-.268-.005-.47.193-.6.224-.135.38-.271.483-.336.104-.074.143-.102.176-.131h.002v-.003c.169-.202.436-.47.839-.601.139-.036.294-.065.466-.065zm2.8 2.142c.358 1.417 1.196 3.475 1.735 4.473.286.534.855 1.659 1.102 3.024.156-.005.33.018.513.064.646-1.671-.546-3.467-1.089-3.966-.22-.2-.232-.335-.123-.335.59.534 1.365 1.572 1.646 2.757.13.535.16 1.104.021 1.67.067.028.135.06.205.067 1.032.534 1.413.938 1.23 1.537v-.043c-.06-.003-.12 0-.18 0h-.016c.151-.467-.182-.825-1.065-1.224-.915-.4-1.646-.336-1.77.465-.008.043-.013.066-.018.135-.068.023-.139.053-.209.064-.43.268-.662.669-.793 1.187-.13.533-.17 1.156-.205 1.869v.003c-.02.334-.17.838-.319 1.35-1.5 1.072-3.58 1.538-5.348.334a2.645 2.645 0 00-.402-.533 1.45 1.45 0 00-.275-.333c.182 0 .338-.03.465-.067a.615.615 0 00.314-.334c.108-.267 0-.697-.345-1.163-.345-.467-.931-.995-1.788-1.521-.63-.4-.986-.87-1.15-1.396-.165-.534-.143-1.085-.015-1.645.245-1.07.873-2.11 1.274-2.763.107-.065.037.135-.408.974-.396.751-1.14 2.497-.122 3.854a8.123 8.123 0 01.647-2.876c.564-1.278 1.743-3.504 1.836-5.268.048.036.217.135.289.202.218.133.38.333.59.465.21.201.477.335.876.335.039.003.075.006.11.006.412 0 .73-.134.997-.268.29-.134.52-.334.74-.4h.005c.467-.135.835-.402 1.044-.7zm2.185 8.958c.037.6.343 1.245.882 1.377.588.134 1.434-.333 1.791-.765l.211-.01c.315-.007.577.01.847.268l.003.003c.208.199.305.53.391.876.085.4.154.78.409 1.066.486.527.645.906.636 1.14l.003-.007v.018l-.003-.012c-.015.262-.185.396-.498.595-.63.401-1.746.712-2.457 1.57-.618.737-1.37 1.14-2.036 1.191-.664.053-1.237-.2-1.574-.898l-.005-.003c-.21-.4-.12-1.025.056-1.69.176-.668.428-1.344.463-1.897.037-.714.076-1.335.195-1.814.12-.465.308-.797.641-.984l.045-.022zm-10.814.049h.01c.053 0 .105.005.157.014.376.055.706.333 1.023.752l.91 1.664.003.003c.243.533.754 1.064 1.189 1.637.434.598.77 1.131.729 1.57v.006c-.057.744-.48 1.148-1.125 1.294-.645.135-1.52.002-2.395-.464-.968-.536-2.118-.469-2.857-.602-.369-.066-.61-.2-.723-.4-.11-.2-.113-.602.123-1.23v-.004l.002-.003c.117-.334.03-.752-.027-1.118-.055-.401-.083-.71.043-.94.16-.334.396-.4.69-.533.294-.135.64-.202.915-.47h.002v-.002c.256-.268.445-.601.668-.838.19-.201.38-.336.663-.336zm7.159-9.074c-.435.201-.945.535-1.488.535-.542 0-.97-.267-1.28-.466-.154-.134-.28-.268-.373-.335-.164-.134-.144-.333-.074-.333.109.016.129.134.199.2.096.066.215.2.36.333.292.2.68.467 1.167.467.485 0 1.053-.267 1.398-.466.195-.135.445-.334.648-.467.156-.136.149-.267.279-.267.128.016.034.134-.147.332a8.097 8.097 0 01-.69.468zm-1.082-1.583V5.64c-.006-.02.013-.042.029-.05.074-.043.18-.027.26.004.063 0 .16.067.15.135-.006.049-.085.066-.135.066-.055 0-.092-.043-.141-.068-.052-.018-.146-.008-.163-.065zm-.551 0c-.02.058-.113.049-.166.066-.047.025-.086.068-.14.068-.05 0-.13-.02-.136-.068-.01-.066.088-.133.15-.133.08-.031.184-.047.259-.005.019.009.036.03.03.05v.02h.003z"/></svg><span style="display:flex;flex-direction:column;line-height:16px"><span style="font-size:11.5px;color:var(--color-muted, oklch(0.708 0 0))">Linux, x86_64 or aarch64</span><span style="display:none">: </span><span style="font-size:13px;color:var(--color-fg, oklch(0.97 0 0))">glibc ≥ 2.35</span></span></li>
<li style="display:flex;align-items:center;gap:7px;min-height:32px;box-sizing:border-box;margin:0;padding:5px 8px;border:1px solid var(--color-border-strong, oklch(0.371 0 0));border-radius:4px;background:var(--color-code-bg, oklch(0.145 0 0));color:var(--color-fg, oklch(0.97 0 0));text-decoration:none"><svg aria-hidden="true" width="16" height="16" viewBox="0 0 24 24" fill="currentColor" style="display:block;flex:none;color:var(--color-muted, oklch(0.708 0 0))"><path d="M3 15.731c1.968 1.507 5.234 2.269 9 2.269c3.76 0 7.025 -.76 9 -2.252v2.252c0 2.425 -3.895 3.936 -8.693 3.998l-.307 .002c-4.938 0 -9 -1.523 -9 -4z"/><path d="M3 9.731c1.968 1.507 5.234 2.269 9 2.269c3.76 0 7.025 -.76 9 -2.252v2.252c0 2.477 -4.062 4 -9 4c-4.798 0 -8.77 -1.438 -8.979 -3.795l-.016 -.101l-.005 -.104z"/><path d="M12 2c1.041 0 2.044 .068 2.977 .198l.469 .071q .84 .14 1.586 .348l.44 .131l.075 .024a11 11 0 0 1 .805 .3l.199 .086q .535 .242 .967 .53q .165 .11 .313 .225a3.8 3.8 0 0 1 .669 .668l.091 .128q .07 .105 .129 .211l.07 .139q .163 .35 .2 .73l.01 .211c0 2.477 -4.062 4 -9 4c-4.798 0 -8.77 -1.438 -8.979 -3.795a1 1 0 0 1 -.021 -.205l.005 -.104l.016 -.1c.205 -2.306 4.01 -3.733 8.667 -3.794z"/></svg><span style="display:flex;flex-direction:column;line-height:16px"><span style="font-size:11.5px;color:var(--color-muted, oklch(0.708 0 0))">Disk</span><span style="display:none">: </span><span style="font-size:13px;color:var(--color-fg, oklch(0.97 0 0))">~30 GB</span></span></li>
<li style="margin:0;padding:0"><a href="https://github.com/rerun-io/examples-monorepo/tree/main/packages/posekit" style="display:flex;align-items:center;gap:7px;min-height:32px;box-sizing:border-box;margin:0;padding:5px 8px;border:1px solid var(--color-border-strong, oklch(0.371 0 0));border-radius:4px;background:var(--color-code-bg, oklch(0.145 0 0));color:var(--color-fg, oklch(0.97 0 0));text-decoration:none"><svg aria-hidden="true" width="16" height="16" viewBox="0 0 24 24" fill="currentColor" style="display:block;flex:none;color:var(--color-muted, oklch(0.708 0 0))"><path d="M5.315 2.1c.791 -.113 1.9 .145 3.333 .966l.272 .161l.16 .1l.397 -.083a13.3 13.3 0 0 1 4.59 -.08l.456 .08l.396 .083l.161 -.1c1.385 -.84 2.487 -1.17 3.322 -1.148l.164 .008l.147 .017l.076 .014l.05 .011l.144 .047a1 1 0 0 1 .53 .514a5.2 5.2 0 0 1 .397 2.91l-.047 .267l-.046 .196l.123 .163c.574 .795 .93 1.728 1.03 2.707l.023 .295l.007 .272c0 3.855 -1.659 5.883 -4.644 6.68l-.245 .061l-.132 .029l.014 .161l.008 .157l.004 .365l-.002 .213l-.003 3.834a1 1 0 0 1 -.883 .993l-.117 .007h-6a1 1 0 0 1 -.993 -.883l-.007 -.117v-.734c-1.818 .26 -3.03 -.424 -4.11 -1.878l-.535 -.766c-.28 -.396 -.455 -.579 -.589 -.644l-.048 -.019a1 1 0 0 1 .564 -1.918c.642 .188 1.074 .568 1.57 1.239l.538 .769c.76 1.079 1.36 1.459 2.609 1.191l.001 -.678l-.018 -.168a5.03 5.03 0 0 1 -.021 -.824l.017 -.185l.019 -.12l-.108 -.024c-2.976 -.71 -4.703 -2.573 -4.875 -6.139l-.01 -.31l-.004 -.292a5.6 5.6 0 0 1 .908 -3.051l.152 -.222l.122 -.163l-.045 -.196a5.2 5.2 0 0 1 .145 -2.642l.1 -.282l.106 -.253a1 1 0 0 1 .529 -.514l.144 -.047l.154 -.03z"/></svg><span style="display:flex;flex-direction:column;line-height:16px"><span style="font-size:11.5px;color:var(--color-muted, oklch(0.708 0 0))">GitHub, examples-monorepo</span><span style="display:none">: </span><span style="font-size:13px;color:var(--color-fg, oklch(0.97 0 0));text-decoration:underline;text-decoration-color:var(--color-border-strong, oklch(0.371 0 0));text-underline-offset:3px">packages/posekit</span></span></a></li>
</ul>

### With a coding agent

Paste this prompt into any coding agent, or into the agent panel of the Rerun viewer.
It checks your machine, asks before it installs anything, and runs the example.

```agent-prompt
Install and run the Rerun EgoHand example on this machine: NVIDIA's EgoHand finds both hands in a head-camera video and logs 3D joints and hand meshes to the Rerun viewer. Do the steps in order and stop at the first that fails.

1. Check the machine. This example needs Linux (x86_64 or aarch64) with an NVIDIA GPU and a driver for CUDA 13 or newer. Run `uname -sm`, `nvidia-smi` and `ldd --version`. If it is not Linux, `nvidia-smi` is missing, the "CUDA Version" it prints is below 13.0, or glibc is older than 2.35, stop: tell me why, and that I can try the example in the browser at https://pablovela5620-egohand-rerun.hf.space instead.
2. Check Pixi with `pixi --version`; it must be 0.83 or newer. Use Pixi for everything; do not use pip, uv or conda.
3. Choose where the code goes. Propose `~/rerun-examples/examples-monorepo` (not /tmp: the environment and model weights need about 30 GB and must survive a reboot). Run `df -h` on its nearest existing parent (e.g. `df -h ~`), because the folder does not exist yet. Then ask me ONE question that covers everything: the folder, the free space there, and, if needed, that you will install Pixi (`curl -fsSL https://pixi.sh/install.sh | sh`) or update it (`pixi self-update`). Wait for my answer, and use another folder if I name one. If less than 30 GB is free there, say so in the question.
4. Get the code. If the folder already holds a clone of https://github.com/rerun-io/examples-monorepo, run `git pull` in it; otherwise `git clone --depth 1 https://github.com/rerun-io/examples-monorepo.git <folder>`. Then `cd <folder>`. If you installed Pixi, open a new shell (or source the shell rc) so `pixi` is on PATH.
5. Pick where the output goes, then run. Base command: `pixi run --frozen -e posekit-hands posekit-video-hands --video-path assets/examples/genhumanego_kettle.mp4 <output flags> egohand onnx egohand` (the output flags go before `egohand onnx egohand`). The first run installs a large CUDA environment and downloads public weights from Hugging Face, so it takes a while.
   - If you have Rerun viewer tools (you run inside the Rerun viewer's agent panel): send the data to that viewer. Call `rerun_connect` with no endpoint; its reply names the viewer's address, e.g. `http://127.0.0.1:41881`. Use `--rr-config.connect --rr-config.port <that port>`. The video, hand boxes, 2D joints and 3D hands then appear in the viewer you are in.
   - Otherwise, with a display: no output flags. The command spawns a Rerun viewer on port 9876. If a Rerun viewer already listens there, the data shows in that viewer. If something else listens there (`ss -ltnp | grep ':9876 '`), for example a `rerun server`, it swallows the data and you see nothing: then add `--rr-config.port <a free port>`.
   - Otherwise, with no display (for example over SSH, when `DISPLAY` and `WAYLAND_DISPLAY` are both empty): `--rr-config.save <folder>/egohand.rrd` with an absolute path (the task runs in `packages/posekit`, so a relative path lands there), and tell me to open that file with the Rerun viewer on a machine with a screen (`pixi run --frozen -e posekit-hands rerun <folder>/egohand.rrd`, or https://rerun.io/viewer).
6. Check that the data arrived: the output must not contain `Failed to flush sink` or `connection severed`. If it does, the port belongs to something that is not a viewer; fix the port as in step 5 and run again. Inside the viewer's agent panel, also call `rerun_get_state` and check that a `video_hands` recording with a `frame` timeline is open.
7. When it finishes, tell me what ran, how long it took, the folder and its size on disk (`du -sh <folder>`), that deleting that folder removes the code and its environment (the downloaded packages stay in Pixi's cache and the model weights in `~/.cache/huggingface`, where other projects can reuse them), and that I can pass `--video-path <my video.mp4>` to run my own clip.
```

### Manually

Install [Pixi](https://pixi.sh/latest/#installation) 0.83 or newer, then:

```sh
git clone --depth 1 https://github.com/rerun-io/examples-monorepo.git
cd examples-monorepo
pixi run --frozen -e posekit-hands posekit-video-hands --video-path assets/examples/genhumanego_kettle.mp4 egohand onnx egohand
```

The first run installs the environment and downloads the model weights.
It opens the Rerun viewer on a 6 s Gen-HumanEgo clip, already remapped from the fisheye into a 65° pinhole view, so the default intrinsics are exact.
To run your own video, change `--video-path`, and pass `--fx/--fy/--cx/--cy` (or `--hfov-deg`): the 3D placement depends on them.

This runs the reference stack, which works on any supported GPU.
For the fast path in the table below, replace `egohand onnx egohand` with `egohand-fast tensorrt egohand-fast` on a Blackwell GPU, or with `egohand-fast-bf16 tensorrt egohand-fast-bf16` on older GPUs.
The first fast run builds its TensorRT engines, which takes a few minutes.

## The result

The video above is 30 s of a [Gen-HumanEgo](https://huggingface.co/datasets/genrobot2025/Gen-HumanEgo) head camera: a person hangs shirts on hangers.
Left: the fisheye frame with boxes and 2D joints.
Right: 3D joints and SOMA hand meshes in the camera frame.
Both hands are found on all 900 frames.
The CLI takes a pinhole camera, so this run gives it the lens's focal length and center without the double-sphere distortion: the 3D placement is an approximation. Steps 1 and 3 below show the real double-sphere lens.

The same 900 frames, one at a time, with detection on every frame and full meshes (decode not timed), on an NVIDIA DGX Spark (GB10):

| Config (this 900-frame clip) | ms per frame | fps | Speed-up |
|---|---|---|---|
| Reference (`egohand`) | 187.56 | 5.3 | 1.00x |
| Fast path (`egohand-fast`: TensorRT, NVFP4/FP8 backbone, CUDA graphs) | 18.25 | 54.8 | 10.28x |

Where the time goes, per stage, reference (left) against the fast path (right):

<picture>
  <img src="https://static.rerun.io/egohand_seg_stages/b963bf82fd6b4a614d084259d3ba96a84271968d/full.webp" alt="Runtime per stage: reference against fast path">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/egohand_seg_stages/b963bf82fd6b4a614d084259d3ba96a84271968d/480w.webp">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/egohand_seg_stages/b963bf82fd6b4a614d084259d3ba96a84271968d/768w.webp">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/egohand_seg_stages/b963bf82fd6b4a614d084259d3ba96a84271968d/1024w.webp">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/egohand_seg_stages/b963bf82fd6b4a614d084259d3ba96a84271968d/1200w.webp">
</picture>

## How one frame goes through it

Frame 34 of the run above: both hands hold a white shirt on a hanger.

<picture>
  <img src="https://static.rerun.io/egohand_egohand_flow/860a9b8e0f4a213c74b4d13be34021174daee28a/full.webp" alt="The pipeline on frame 34: camera frame, RT-DETR hand detector, hand crops, DINOv3, heads, MHR / SOMA mesh">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/egohand_egohand_flow/860a9b8e0f4a213c74b4d13be34021174daee28a/480w.webp">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/egohand_egohand_flow/860a9b8e0f4a213c74b4d13be34021174daee28a/768w.webp">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/egohand_egohand_flow/860a9b8e0f4a213c74b4d13be34021174daee28a/1024w.webp">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/egohand_egohand_flow/860a9b8e0f4a213c74b4d13be34021174daee28a/1200w.webp">
</picture>

### 1. The camera frame and its lens

The double-sphere lens sees 163° across, so straight lines bend. EgoHand never undistorts the whole frame, only one small view per hand.

<picture>
  <img src="https://static.rerun.io/egohand_walk_1_frame/c836401623cb8297a5c752e8d14f920855b3efb7/full.webp" alt="The camera frame and its lens">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/egohand_walk_1_frame/c836401623cb8297a5c752e8d14f920855b3efb7/480w.webp">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/egohand_walk_1_frame/c836401623cb8297a5c752e8d14f920855b3efb7/768w.webp">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/egohand_walk_1_frame/c836401623cb8297a5c752e8d14f920855b3efb7/1024w.webp">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/egohand_walk_1_frame/c836401623cb8297a5c752e8d14f920855b3efb7/1200w.webp">
</picture>

### 2. Find the hands

An [RT-DETR](https://arxiv.org/abs/2304.08069) hand detector sees a 640 × 640 copy of the frame and returns a box and a side for each hand.

<picture>
  <img src="https://static.rerun.io/egohand_walk_2_detector/9eb0eff018ce425e3e8f6772e96fdd626b6ba72a/full.webp" alt="Find the hands">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/egohand_walk_2_detector/9eb0eff018ce425e3e8f6772e96fdd626b6ba72a/480w.webp">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/egohand_walk_2_detector/9eb0eff018ce425e3e8f6772e96fdd626b6ba72a/768w.webp">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/egohand_walk_2_detector/9eb0eff018ce425e3e8f6772e96fdd626b6ba72a/1024w.webp">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/egohand_walk_2_detector/9eb0eff018ce425e3e8f6772e96fdd626b6ba72a/1200w.webp">
</picture>

### 3. Crop each hand through the lens

A virtual pinhole camera turns toward each hand and samples the frame through the lens, so the crops come out straight. The right crop is mirrored, so the model always sees a left hand.

<picture>
  <img src="https://static.rerun.io/egohand_walk_3_crops/6669a2d2e6ee2d878fb9e41f3c64c6ea695276ad/full.webp" alt="Crop each hand through the lens">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/egohand_walk_3_crops/6669a2d2e6ee2d878fb9e41f3c64c6ea695276ad/480w.webp">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/egohand_walk_3_crops/6669a2d2e6ee2d878fb9e41f3c64c6ea695276ad/768w.webp">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/egohand_walk_3_crops/6669a2d2e6ee2d878fb9e41f3c64c6ea695276ad/1024w.webp">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/egohand_walk_3_crops/6669a2d2e6ee2d878fb9e41f3c64c6ea695276ad/1200w.webp">
</picture>

### 4. Backbone features

[DINOv3](https://arxiv.org/abs/2508.10104) ViT-H+/16 turns each crop into 256 tokens through 32 blocks.

<picture>
  <img src="https://static.rerun.io/egohand_walk_4_backbone/cb1c4dfa2b40136905349dc7210e3a7e1bd774f1/full.webp" alt="DINOv3 features">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/egohand_walk_4_backbone/cb1c4dfa2b40136905349dc7210e3a7e1bd774f1/480w.webp">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/egohand_walk_4_backbone/cb1c4dfa2b40136905349dc7210e3a7e1bd774f1/768w.webp">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/egohand_walk_4_backbone/cb1c4dfa2b40136905349dc7210e3a7e1bd774f1/1024w.webp">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/egohand_walk_4_backbone/cb1c4dfa2b40136905349dc7210e3a7e1bd774f1/1200w.webp">
</picture>

### 5. Heads: pose, shape, depth and 2D joints

The heads give 25 joint rotations, the hand's shape, a depth and 25 direct 2D joints per hand. The 2D joints place the hand sideways.

<picture>
  <img src="https://static.rerun.io/egohand_walk_5_heads/5183514109ffa6face065b5b0ba2a5e46ee4e96c/full.webp" alt="Heads: pose, shape, depth and 2D joints">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/egohand_walk_5_heads/5183514109ffa6face065b5b0ba2a5e46ee4e96c/480w.webp">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/egohand_walk_5_heads/5183514109ffa6face065b5b0ba2a5e46ee4e96c/768w.webp">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/egohand_walk_5_heads/5183514109ffa6face065b5b0ba2a5e46ee4e96c/1024w.webp">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/egohand_walk_5_heads/5183514109ffa6face065b5b0ba2a5e46ee4e96c/1200w.webp">
</picture>

### 6. Metric hand mesh

[SOMA](https://arxiv.org/abs/2603.16858) turns pose and shape into a 2,859-vertex mesh per hand, in metres in the camera frame.

<picture>
  <img src="https://static.rerun.io/egohand_walk_6_mesh/46d27cffbc79afe08546723f829df95497fcc497/full.webp" alt="Metric hand mesh">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/egohand_walk_6_mesh/46d27cffbc79afe08546723f829df95497fcc497/480w.webp">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/egohand_walk_6_mesh/46d27cffbc79afe08546723f829df95497fcc497/768w.webp">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/egohand_walk_6_mesh/46d27cffbc79afe08546723f829df95497fcc497/1024w.webp">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/egohand_walk_6_mesh/46d27cffbc79afe08546723f829df95497fcc497/1200w.webp">
</picture>

### 7. Output

Boxes, 2D joints on the video, 3D joints and meshes, logged to Rerun for every frame.

<picture>
  <img src="https://static.rerun.io/egohand_walk_7_output/e5f16bf9ed5df680a185b16acb2d33925d0046e7/full.webp" alt="Output">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/egohand_walk_7_output/e5f16bf9ed5df680a185b16acb2d33925d0046e7/480w.webp">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/egohand_walk_7_output/e5f16bf9ed5df680a185b16acb2d33925d0046e7/768w.webp">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/egohand_walk_7_output/e5f16bf9ed5df680a185b16acb2d33925d0046e7/1024w.webp">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/egohand_walk_7_output/e5f16bf9ed5df680a185b16acb2d33925d0046e7/1200w.webp">
</picture>

## How it works

EgoHand is a single-camera model: DINOv3 features with ray encoding feed a 3D hand decoder (SOMA hand pose, [MHR](https://arxiv.org/abs/2511.15586) shape, depth) and a 2D joint decoder, and a closed-form solve places the wrist.
It was trained on about 14 M images.
The fast path keeps the same outputs: it runs the detector and the backbone as TensorRT engines (NVFP4 and FP8 on Blackwell GPUs), crops on the GPU and replays CUDA graphs.

## Papers and credits

* [EgoHand: Monocular 3D Hand Reconstruction from Egocentric Video](https://arxiv.org/abs/2610.30293), Chen et al. (NVIDIA), with [code](https://github.com/NVIDIA/IsaacCapture/tree/main/src/python/isaaccapture_egopost) and [weights](https://huggingface.co/nvidia/EgoHand-v1).
* [RT-DETR](https://arxiv.org/abs/2304.08069), Zhao et al., and [RT-DETRv2](https://arxiv.org/abs/2407.17140), Lv et al.
* [DINOv3](https://arxiv.org/abs/2508.10104), Siméoni et al. (Meta).
* [MHR: Momentum Human Rig](https://arxiv.org/abs/2511.15586), Ferguson et al. (Meta).
* [SOMA: Unifying Parametric Human Body Models](https://arxiv.org/abs/2603.16858), Saito et al. (NVIDIA), with [SOMA-X](https://github.com/NVlabs/SOMA-X).
* [The Double Sphere Camera Model](https://arxiv.org/abs/1807.08957), Usenko, Demmel and Cremers.
* Weights: [EgoHand-v1](https://huggingface.co/nvidia/EgoHand-v1) and [RT-DETR-Hand-Detector-v1](https://huggingface.co/nvidia/RT-DETR-Hand-Detector-v1) (NVIDIA, OpenMDW-1.1; the example loads EgoHand from a weights-only mirror, [pablovela5620/egohand](https://huggingface.co/pablovela5620/egohand)), and [SOMA-X](https://huggingface.co/nvidia/SOMA-X) (Apache-2.0).
* Video: [Gen-HumanEgo](https://huggingface.co/datasets/genrobot2025/Gen-HumanEgo) ([CC BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/)). The video, images and recording on this page overlay boxes, joints and hand meshes on it, and are shared under the same licence.
