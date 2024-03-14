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

Visualize the [BERT-based named entity recognition (NER)](https://huggingface.co/dslim/bert-base-NER). 

## Used Rerun Types
[`TextDocument`](https://www.rerun.io/docs/reference/types/archetypes/text_document), [`AnnotationContext`](https://www.rerun.io/docs/reference/types/archetypes/annotation_context), [`Points3D`](https://www.rerun.io/docs/reference/types/archetypes/points3d)

## Background
It works by splitting text into tokens, feeding the token sequence into a large language model (BERT) to retrieve embeddings per token. The embeddings are then classified.

# Run the Code
To run this example, make sure you have the Rerun repository checked out and the latest SDK installed:
```bash
# Setup 
pip install --upgrade rerun-sdk  # install the latest Rerun SDK
git clone git@github.com:rerun-io/rerun.git  # Clone the repository
cd rerun
git checkout latest  # Check out the commit matching the latest SDK release
```
Install the necessary libraries specified in the requirements file:
```bash
pip install -r examples/python/llm_embedding_ner/requirements.txt
```
To experiment with the provided example, simply execute the main Python script:
```bash
python examples/python/llm_embedding_ner/main.py # run the example
```
If you wish to customize it, explore additional features, or save it use the CLI with the `--help` option for guidance:
```bash
python examples/python/llm_embedding_ner/main.py --help 

usage: main.py [-h] [--text TEXT] [--headless] [--connect] [--serve] [--addr ADDR] [--save SAVE] [-o]

BERT-based named entity recognition (NER)

optional arguments:
  -h, --help    show this help message and exit
  --text TEXT   Text that is processed.
  --headless    Don t show GUI
  --connect     Connect to an external viewer
  --serve       Serve a web viewer (WARNING: experimental feature)
  --addr ADDR   Connect to this ip:port
  --save SAVE   Save data to a .rrd file at this path
  -o, --stdout  Log data to standard output, to be piped into a Rerun Viewer
```