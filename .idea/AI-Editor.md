# AI editor — model study

Choosing the models for an editor that removes objects, cuts out backgrounds,
upscales and restores, on a laptop with an RTX 3050 Ti (4 GB) and a Ryzen 7
4800H.

Every size below was read from the Hugging Face API, not remembered. Every
licence was read from the repo metadata or, where the repo declares none, from
the upstream project.

---

## 1. The constraint is not the one we assumed

Hive does not use the GPU. At all.

```toml
ort = "=2.0.0-rc.12"     # no features → CPU execution provider
```

No `execution_providers` call exists anywhere in `src-tauri/src`. CLIP, OCR,
face recognition, captioning and NSFW scoring all run on the CPU. The 4 GB of
VRAM has been sitting idle this whole time.

So "will it fit in 4 GB of VRAM" is the wrong question until we answer a prior
one: **do we turn the GPU on?**

### Turning it on

`ort` 2.0.0-rc.12 ships these providers as cargo features:

```
cuda   directml   tensorrt   rocm   openvino   coreml   webgpu   xnnpack   onednn
```

Two are candidates on this machine.

| | CUDA | DirectML |
|---|---|---|
| Works on the RTX 3050 Ti | yes | yes |
| Works on the integrated Radeon | no | yes |
| Works on a user with an AMD or Intel GPU | no | yes |
| What ships with the app | cuDNN + CUDA runtime, well over 1 GB | one DLL, ~10 MB |
| Needs the user to install anything | CUDA toolkit | no, it is part of Windows 10+ |
| Speed on this GPU | fastest | ~10–30% behind CUDA |

**DirectML wins**, and not narrowly. Hive is a local-first app meant to be
installed by ordinary people. Shipping a gigabyte of NVIDIA runtime, or telling
someone to install a toolkit before they can cut out a background, defeats the
premise. DirectML is a Windows component, works on every DX12 GPU, and falls
back gracefully.

The CPU path stays as the fallback: same models, same code, slower. That matters
because it is also the path on a machine with no usable GPU at all.

### What this does to the model budget

With DirectML on, 4 GB is a real ceiling — but a soft one, because image models
are **tiled**. An upscaler does not process a 12-megapixel photo in one go; it
walks it in 512×512 tiles. Peak memory tracks the tile, not the photo. A 4 GB
card running 512-pixel tiles is comfortable for everything in this study.

The hard limit is the model's own weights, which stay resident. That is what the
size column below is really measuring.

---

## 2. Category by category

### 2.1 Object removal (inpainting)

The flagship. Paint over a person, they disappear and the background is invented
behind them.

| Model | ONNX | Size | Licence | Verdict |
|---|---|---|---|---|
| **LaMa** (`Carve/LaMa-ONNX`) | yes | **207 MB** | Apache 2.0 | **chosen** |
| MI-GAN (`anyisalin/migan-onnx`) | yes | 28 MB | undeclared | rejected — licence |
| MAT (`Acly/MAT`) | no ONNX | — | CC-BY-NC | rejected |

LaMa is the reference for this task and has been since 2021. Its trick is
Fourier convolutions, which give every layer a view of the whole image — that is
why it can continue a brick wall or a horizon across a large hole, where
ordinary convolutional inpainters smear.

MI-GAN is tempting at 28 MB and is genuinely good, but the repo declares no
licence. An undeclared licence is not a permissive one: it means all rights
reserved. Not something to build a feature on.

207 MB is the largest download in this study, and it buys the feature people
will actually judge the editor by.

### 2.2 Selection (interactive segmentation)

Inpainting needs a mask. Asking someone to paint precisely around a person with
a mouse is miserable. SAM turns that into one click.

| Model | Size (quantised) | Downloads | Licence | Verdict |
|---|---|---|---|---|
| **SlimSAM** (`Xenova/slimsam-77-uniform`) | **13.8 MB** total | 50,867 | Apache 2.0 | **chosen** |
| MobileSAM (community exports) | ~16–28 MB | 0–28 | Apache 2.0 | rejected — unmaintained |
| SAM ViT-B | ~350 MB | — | Apache 2.0 | rejected — 25× the size |

SlimSAM is SAM pruned to 0.1% of its original parameters while keeping most of
its accuracy. The split matters for how it is used:

```
vision_encoder_quantized.onnx            8.9 MB   runs once per photo
prompt_encoder_mask_decoder_quantized    4.9 MB   runs on every click
```

Encode the photo once, then each click costs only the 4.9 MB decoder — a few
milliseconds. That is what makes click-to-select feel instant rather than like
a request.

The Xenova export is the one transformers.js uses, with 50k downloads behind it.
The MobileSAM community exports have essentially none, and an ONNX export nobody
runs is an ONNX export nobody has debugged.

### 2.3 Background removal

| Model | Size | Downloads | Licence | Verdict |
|---|---|---|---|---|
| RMBG-1.4 (`briaai`) | 44 MB q8 | 304,137 | **BRIA non-commercial** | **rejected — licence** |
| **U²-Net** (`isnet-general-use`) | **178 MB** | — | Apache 2.0 | **chosen** |
| MODNet (`Xenova/modnet`) | 6.6 MB q8 | 64,507 | Apache 2.0 | chosen as the portrait path |
| BiRefNet | 490 MB fp16 | 803 | MIT | rejected — size |

RMBG-1.4 is the popular answer and the wrong one here. Its licence forbids
commercial use. Hive may never be sold, but baking a non-commercial dependency
into it closes that door permanently, and quietly.

The honest replacement is two models rather than one, because they are good at
different things:

- **MODNet, 6.6 MB** — portrait matting. Trained on humans, and it resolves hair
  strand by strand, which the general models turn into a blob. Six megabytes.
- **ISNet / U²-Net, 178 MB** — anything else. Products, animals, objects.

Picking between them automatically is easy: Hive already runs face detection.
A face in the frame means the portrait model.

### 2.4 Upscaling

| Model | Size | Licence | Verdict |
|---|---|---|---|
| **Real-ESRGAN general x4v3** (`Heliosoph`) | **4.9 MB** | BSD-3-Clause | **chosen** |
| Real-ESRGAN x4plus | 33.6 MB fp16 / 67 MB | BSD-3-Clause | fallback for hard cases |
| Swin2SR (`Xenova`) | 21.5 MB q8 | Apache 2.0 | rejected — ×2 only, slower |
| Real-ESRGAN anime 6B | 18.4 MB | BSD-3-Clause | out of scope |

`realesr-general-x4v3` is the compact variant its authors built for real-world
photographs — the ones with JPEG artefacts and sensor noise, not clean
downsampled test images. At **4.9 MB** it is smaller than a photo, and it is the
best value in this entire study.

Swin2SR is technically stronger on benchmark images but it is a transformer:
slower, and ×2 where the ESRGAN is ×4.

### 2.5 Face restoration

| Model | ONNX | Size | Licence | Verdict |
|---|---|---|---|---|
| CodeFormer (`bluefoxcreation`) | yes | 377 MB | **NTU S-Lab non-commercial** | **rejected — licence** |
| GFPGAN | no credible export found | — | Apache 2.0 upstream | rejected — no ONNX |

**Nothing here is usable.** The only real ONNX export is CodeFormer, whose
licence bars commercial use exactly like RMBG. GFPGAN's licence would be fine,
but no maintained ONNX export exists — the repos that turn up hold `.pth`
weights or unrelated files.

Deferred rather than substituted. The upscaler already improves faces somewhat,
and shipping a licence problem to avoid an empty slot is a bad trade.

### 2.6 Colourisation

**No usable ONNX export exists.** Searches for DDColor and DeOldify return
nothing with ONNX weights.

Both are PyTorch projects that would have to be exported by hand — a day of work
with no guarantee the export is faithful, which the captioning decoder already
taught us is a real risk. Deferred.

### 2.7 Object detection

| Model | Size | Licence | Verdict |
|---|---|---|---|
| YOLOv10n (`onnx-community`) | 2.7 MB | **AGPL-3.0** | **rejected — licence** |
| YOLOS-tiny (`Xenova`) | 9.7 MB q8 | Apache 2.0 | available if needed |
| RT-DETR r18 (`onnx-community`) | 21.7 MB q8 | Apache 2.0 | available if needed |

AGPL-3.0 is viral: it would oblige Hive to publish its own source under AGPL.
Every Ultralytics YOLO carries it. This is the trap in the suggestion list.

But the deeper point: **this feature is not needed.** Its purpose was to let the
user pick an object to remove, and SlimSAM already does that better — a click
selects anything, not just the 80 classes a detector was trained on. Dropped, in
favour of the thing that supersedes it.

### 2.8 Sky replacement

Not a model — segmentation plus compositing, and the compositing is where the
difficulty lives. A pasted sky looks pasted unless the foreground's colour
temperature is relit to match it.

Real value, but it is an afternoon of colour work on top of everything else.
Deferred to after the rest is solid.

---

## 3. What gets built

| Feature | Model | Download |
|---|---|---|
| Click-to-select | SlimSAM | 13.8 MB |
| Remove object | LaMa | 207 MB |
| Remove background — portrait | MODNet | 6.6 MB |
| Remove background — general | ISNet / U²-Net | 178 MB |
| Upscale ×4 | Real-ESRGAN general v3 | 4.9 MB |

**410 MB total**, and each one downloads only when its feature is first used —
the same pattern as the existing models. Someone who only ever upscales pays
4.9 MB.

Every licence is Apache 2.0 or BSD-3: no non-commercial clause, no AGPL.

### Deferred, with reasons

| Feature | Why |
|---|---|
| Face restoration | Only ONNX export is non-commercial |
| Colourisation | No ONNX export exists; needs a hand export |
| Sky replacement | Compositing work, not a model problem |
| Object detection | Superseded by click-to-select |

### Order of work

1. **DirectML** — unlocks the GPU for every model in the app, new and old
2. **Upscale** — 4.9 MB, one pass, no user input; proves the tiling
3. **Background removal** — one pass, no user input; two models, chosen by face count
4. **Object removal** — SlimSAM then LaMa; the flagship, and the one with a real UI

Upscale first is deliberate. It is the smallest model and the simplest pipeline,
so it flushes out the plumbing — GPU provider, tiling, progress, cancellation —
while the thing being debugged is 4.9 MB rather than 207.
