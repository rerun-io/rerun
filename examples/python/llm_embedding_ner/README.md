<!--[metadata]
title = "LLM Embedding-Based Named Entity Recognition"
tags = ["LLM", "embeddings", "classification", "huggingface", "text"]
thumbnail = "https://static.rerun.io/llm_embedding_ner/d98c09dd6bfa20ceea3e431c37dc295a4009fa1b/480w.png"
thumbnail_dimensions = [480, 279]
-->

<picture>
  <img src="https://static.rerun.io/llm_embedding_ner/d98c09dd6bfa20ceea3e431c37dc295a4009fa1b/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/llm_embedding_ner/d98c09dd6bfa20ceea3e431c37dc295a4009fa1b/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/llm_embedding_ner/d98c09dd6bfa20ceea3e431c37dc295a4009fa1b/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/llm_embedding_ner/d98c09dd6bfa20ceea3e431c37dc295a4009fa1b/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/llm_embedding_ner/d98c09dd6bfa20ceea3e431c37dc295a4009fa1b/1200w.png">
</picture>

This example visualizes [BERT-based named entity recognition (NER)](https://huggingface.co/dslim/bert-base-NER). It works by splitting text into tokens, feeding the token sequence into a large language model (BERT) to retrieve embeddings per token. The embeddings are then classified.

To run this example use
```bash
pip install -r examples/python/llm_embedding_ner/requirements.txt
python examples/python/llm_embedding_ner/main.py
```

You can specify your own text using
```bash
main.py [--text TEXT]
```
