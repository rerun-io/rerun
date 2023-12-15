#!/usr/bin/env python3
"""
Example running BERT-based named entity recognition (NER).

Run
```sh
examples/python/llm_embedding_ner/main.py
```
"""
from __future__ import annotations

import argparse
from collections import defaultdict
from typing import Any

import rerun as rr
import torch
import umap
from transformers import AutoModelForTokenClassification, AutoTokenizer, pipeline

DEFAULT_TEXT = """
In the bustling city of Brightport, nestled between rolling hills and a sparkling harbor, lived three friends: Maya, a spirited chef known for her spicy curries; Leo, a laid-back jazz musician with a penchant for saxophone solos; and Ava, a tech-savvy programmer who loved solving puzzles.

One sunny morning, the trio decided to embark on a mini-adventure to the legendary Hilltop Café in the nearby town of Greendale. The café, perched on the highest hill, was famous for its panoramic views and delectable pastries.

Their journey began with a scenic drive through the countryside, with Leo's smooth jazz tunes setting a relaxing mood. Upon reaching Greendale, they found the town buzzing with excitement over the annual Flower Festival. The streets were adorned with vibrant blooms, and the air was filled with a sweet floral scent.

At the Hilltop Café, they savored buttery croissants and rich coffee, laughing over past misadventures and dreaming up future plans. The view from the café was breathtaking, overlooking the patchwork of fields and the distant Brightport skyline.

After their café indulgence, they joined the festival's flower parade. Maya, with her knack for colors, helped design a stunning float decorated with roses and lilies. Leo entertained the crowd with his saxophone, while Ava captured the day's memories with her camera.

As the sun set, painting the sky in hues of orange and purple, the friends returned to Brightport, their hearts full of joy and their minds brimming with new memories. They realized that sometimes, the simplest adventures close to home could be the most memorable.
"""


def log_tokenized_text(token_words: list[str]) -> None:
    markdown = ""
    for i, token_word in enumerate(token_words):
        if token_word.startswith("##"):
            clean_token_word = token_word[2:]
        else:
            clean_token_word = " " + token_word

        markdown += f"[{clean_token_word}](recording://umap_embeddings[#{i}])"
    rr.log("tokenized_text", rr.TextDocument(markdown, media_type=rr.MediaType.MARKDOWN))


def log_ner_results(ner_results: list[dict[str, Any]]) -> None:
    entity_sets: dict[str, set[str]] = defaultdict(set)

    current_entity_name = None
    current_entity_set = None
    for ner_result in ner_results:
        entity_class = ner_result["entity"]
        word = ner_result["word"]
        if entity_class.startswith("B-"):
            if current_entity_set is not None and current_entity_name is not None:
                current_entity_set.add(current_entity_name)
            current_entity_set = entity_sets[entity_class[2:]]
            current_entity_name = word
        elif current_entity_name is not None:
            if word.startswith("##"):
                current_entity_name += word[2:]
            else:
                current_entity_name += f" {word}"

    named_entities_str = ""
    if "PER" in entity_sets:
        named_entities_str += f"Persons: {', '.join(entity_sets['PER'])}\n\n"
    if "LOC" in entity_sets:
        named_entities_str += f"Locations: {', '.join(entity_sets['LOC'])}\n\n"
    if "ORG" in entity_sets:
        named_entities_str += f"Organizations: {', '.join(entity_sets['ORG'])}\n\n"
    if "MISC" in entity_sets:
        named_entities_str += f"Miscellaneous: {', '.join(entity_sets['MISC'])}\n\n"

    rr.log("named_entities", rr.TextDocument(named_entities_str, media_type=rr.MediaType.MARKDOWN))


def run_llm_ner(text: str) -> None:
    label2index = {
        "B-LOC": 1,
        "I-LOC": 1,
        "B-PER": 2,
        "I-PER": 2,
        "B-ORG": 3,
        "I-ORG": 3,
        "B-MISC": 4,
        "I-MISC": 4,
    }
    annotation_context = [
        (0, "", (20, 20, 20)),
        (1, "Location", (200, 40, 40)),
        (2, "Person", (40, 200, 40)),
        (3, "Organization", (40, 40, 200)),
        (4, "Miscellaneous", (40, 200, 200)),
    ]
    rr.log("/", rr.AnnotationContext(annotation_context))

    # Initialize model
    tokenizer = AutoTokenizer.from_pretrained("dslim/bert-base-NER")
    model = AutoModelForTokenClassification.from_pretrained("dslim/bert-base-NER")
    nlp = pipeline("ner", model=model, tokenizer=tokenizer)

    # Compute intermediate and final output
    token_ids = tokenizer.encode(text)
    token_words = tokenizer.convert_ids_to_tokens(token_ids)
    embeddings = nlp.model.base_model(torch.tensor([token_ids])).last_hidden_state
    ner_results: Any = nlp(text)

    # Visualize in Rerun
    rr.log("text", rr.TextDocument(text, media_type=rr.MediaType.MARKDOWN))
    log_tokenized_text(token_words)
    reducer = umap.UMAP(n_components=3, n_neighbors=4)
    umap_embeddings = reducer.fit_transform(embeddings.numpy(force=True)[0])
    class_ids = [0 for _ in token_words]
    for ner_result in ner_results:
        class_ids[ner_result["index"]] = label2index[ner_result["entity"]]
    rr.log(
        "umap_embeddings",
        rr.Points3D(umap_embeddings, labels=token_words, class_ids=class_ids),
    )
    log_ner_results(ner_results)


def main() -> None:
    parser = argparse.ArgumentParser(description="BERT-based named entity recognition (NER)")
    parser.add_argument(
        "--text",
        type=str,
        help="Text that is processed.",
        default=DEFAULT_TEXT,
    )
    rr.script_add_args(parser)
    args = parser.parse_args()

    rr.script_setup(args, "rerun_example_llm_embedding_ner")
    run_llm_ner(args.text)
    rr.script_teardown(args)


if __name__ == "__main__":
    main()
