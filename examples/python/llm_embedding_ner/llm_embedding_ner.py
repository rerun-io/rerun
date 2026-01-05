#!/usr/bin/env python3
"""Example running BERT-based named entity recognition (NER)."""

from __future__ import annotations

import argparse
from collections import defaultdict
from typing import Any

import rerun as rr
import rerun.blueprint as rrb
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


def entity_per_token(token_words: list[str], ner_results: list[dict[str, Any]]) -> list[str]:
    index_to_entity: dict[int, str] = defaultdict(str)
    current_entity_name = None
    current_entity_indices = []
    for ner_result in ner_results:
        entity_class = ner_result["entity"]
        word = ner_result["word"]
        token_index = ner_result["index"]
        if entity_class.startswith("B-"):
            if current_entity_name is not None:
                print(current_entity_name, current_entity_indices)
                for i in current_entity_indices:
                    index_to_entity[i] = current_entity_name
            current_entity_indices = [token_index]
            current_entity_name = word
        elif current_entity_name is not None:
            current_entity_indices.append(token_index)
            if word.startswith("##"):
                current_entity_name += word[2:]
            else:
                current_entity_name += f" {word}"
    entity_per_token = [index_to_entity[i] for i in range(len(token_words))]
    return entity_per_token


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
    # Define label for classes and set none class color to dark gray
    annotation_context = [
        rr.AnnotationInfo(id=0, color=(30, 30, 30)),
        rr.AnnotationInfo(id=1, label="Location"),
        rr.AnnotationInfo(id=2, label="Person"),
        rr.AnnotationInfo(id=3, label="Organization"),
        rr.AnnotationInfo(id=4, label="Miscellaneous"),
    ]
    rr.log("/", rr.AnnotationContext(annotation_context))

    # Initialize model
    tokenizer = AutoTokenizer.from_pretrained("dslim/bert-base-NER")
    model = AutoModelForTokenClassification.from_pretrained("dslim/bert-base-NER")
    ner_pipeline = pipeline("ner", model=model, tokenizer=tokenizer)  # type: ignore[call-overload]

    # Compute intermediate and final output
    token_ids = tokenizer.encode(text)
    token_words = tokenizer.convert_ids_to_tokens(token_ids)

    print("Computing embeddings and output…")
    # NOTE The embeddings are currently computed twice (next line and as part of the pipeline)
    #  It'd be better to directly log from inside the pipeline to avoid this.
    embeddings = ner_pipeline.model.base_model(torch.tensor([token_ids])).last_hidden_state
    ner_results: Any = ner_pipeline(text)

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
        rr.Points3D(umap_embeddings, class_ids=class_ids),
        rr.AnyValues(**{"Token": token_words, "Named Entity": entity_per_token(token_words, ner_results)}),
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

    rr.script_setup(
        args,
        "rerun_example_llm_embedding_ner",
        default_blueprint=rrb.Horizontal(
            rrb.Vertical(
                rrb.TextDocumentView(origin="/text", name="Original Text"),
                rrb.TextDocumentView(origin="/tokenized_text", name="Tokenized Text"),
                rrb.TextDocumentView(origin="/named_entities", name="Named Entities"),
                row_shares=[3, 2, 2],
            ),
            rrb.Spatial3DView(origin="/umap_embeddings", name="UMAP Embeddings"),
        ),
    )
    run_llm_ner(args.text)
    rr.script_teardown(args)


if __name__ == "__main__":
    main()
