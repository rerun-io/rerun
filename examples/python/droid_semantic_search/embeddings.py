"""SigLIP-2 embedding helpers shared by `ingest.py` and `search.py`.

These are trimmed copies of the helpers in the DROID loader
(`dataplatform/examples/droid/droid-loader/src/droid_loader/embedding_util.py`),
with the `Timer` instrumentation removed so this example stays standalone and
doesn't pull in the `droid_loader` package. The model is the same one the
loader uses to populate `/camera/{role}/embedding`, so query embeddings land in
the same vector space as any pre-computed frame embeddings.
"""

from __future__ import annotations

import os
from pathlib import Path
from typing import TYPE_CHECKING, Any

import torch

# Disable HF `tokenizers` (Rust) parallelism *before* importing transformers. Otherwise,
# once the SigLIP tokenizer has been used and the process later forks (e.g. a DataLoader
# worker), tokenizers prints "the current process just got forked, after parallelism has
# already been used". We only tokenize tiny queries, so parallelism buys nothing here.
# `setdefault` lets a caller still override via the real environment variable.
os.environ.setdefault("TOKENIZERS_PARALLELISM", "false")

from transformers import AutoModel, AutoProcessor  # imported after TOKENIZERS_PARALLELISM is set (above)

if TYPE_CHECKING:
    from PIL.Image import Image

# The concrete SigLIP-2 model/processor that `from_pretrained` returns.
EmbeddingModel = Any
EmbeddingProcessor = Any

# Dual image/text encoder; image and text features share one space, so text
# queries retrieve image frames directly. 768-dim, L2-normalized output.
EMBEDDING_MODEL = "google/siglip2-base-patch16-224"


def _resolve_device(device: str | torch.device | None) -> torch.device:
    if device is not None:
        return torch.device(device)
    if torch.cuda.is_available():
        return torch.device("cuda")
    if torch.backends.mps.is_available():
        return torch.device("mps")
    return torch.device("cpu")


def load_embedding_model(
    cache_dir: str | Path | None = None,
    use_fast: bool = True,
) -> tuple[EmbeddingModel, EmbeddingProcessor]:
    """Load the SigLIP-2 model and its processor."""
    print(f"Loading model '{EMBEDDING_MODEL}'")
    model = AutoModel.from_pretrained(EMBEDDING_MODEL, cache_dir=cache_dir)
    processor = AutoProcessor.from_pretrained(EMBEDDING_MODEL, cache_dir=cache_dir, use_fast=use_fast)
    return model, processor


def get_text_embeddings(
    text: str | list[str],
    model: EmbeddingModel,
    processor: EmbeddingProcessor,
    device: str | torch.device | None = None,
) -> torch.Tensor:
    """Embed one or more strings into the SigLIP-2 space.

    Returns an L2-normalized `[N, 768]` CPU tensor (one row per input string).
    """
    if isinstance(text, str):
        text = [text]
    if not text:
        raise ValueError("Input 'text' must be a non-empty string or list of strings.")

    device = _resolve_device(device)
    model = model.to(device)
    model.eval()

    # SigLIP is trained with a fixed 64-token sequence; it MUST be tokenized with
    # padding="max_length" (max_length=64). With dynamic padding="True" the text
    # embeddings are malformed and text->image retrieval collapses onto a hub image.
    inputs = processor(text=text, return_tensors="pt", padding="max_length", max_length=64, truncation=True).to(device)
    with torch.inference_mode():
        # transformers 5.x: get_text_features returns the full encoder output, not a
        # bare tensor — the embedding is its `pooler_output` (`[N, 768]`).
        features = model.get_text_features(**inputs).pooler_output
        normalized: torch.Tensor = features / features.norm(p=2, dim=-1, keepdim=True)
        return normalized.cpu()


def compute_image_embeddings(
    images: list[Image],
    model: EmbeddingModel,
    processor: EmbeddingProcessor,
    device: str | torch.device | None = None,
    batch_size: int = 64,
) -> torch.Tensor:
    """Embed a list of PIL images into the SigLIP-2 space.

    Returns an L2-normalized `[len(images), 768]` CPU tensor.
    """
    if not images:
        raise ValueError("Input 'images' list cannot be empty.")

    device = _resolve_device(device)
    model = model.to(device)
    model.eval()

    all_embeddings: list[torch.Tensor] = []
    with torch.inference_mode():
        for start in range(0, len(images), batch_size):
            batch = images[start : start + batch_size]
            inputs = processor(images=batch, return_tensors="pt").to(device)
            # transformers 5.x: get_image_features returns the full encoder output, not a
            # bare tensor — the embedding is its `pooler_output` (`[N, 768]`).
            features = model.get_image_features(**inputs).pooler_output
            normalized = features / features.norm(p=2, dim=1, keepdim=True)
            all_embeddings.append(normalized.cpu())

    return torch.cat(all_embeddings, dim=0)
