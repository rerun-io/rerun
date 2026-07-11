from __future__ import annotations

from rerun.catalog._entry import _huggingface_next_link, _resolve_huggingface_rrd_urls_from_tree


def test_resolve_huggingface_rrd_urls_from_tree_filters_and_quotes_paths() -> None:
    tree = [
        {"type": "file", "path": "README.md"},
        {"type": "directory", "path": "episodes"},
        {"type": "file", "path": "episodes/one.rrd"},
        {"type": "file", "path": "episodes/two with space.rrd"},
        {"type": "file", "path": "episodes/two.mp4"},
    ]

    assert _resolve_huggingface_rrd_urls_from_tree("rerun/droid_sample", "main", tree, limit=None) == [
        "https://huggingface.co/datasets/rerun/droid_sample/resolve/main/episodes/one.rrd",
        "https://huggingface.co/datasets/rerun/droid_sample/resolve/main/episodes/two%20with%20space.rrd",
    ]


def test_resolve_huggingface_rrd_urls_from_tree_respects_limit() -> None:
    tree = [
        {"type": "file", "path": "one.rrd"},
        {"type": "file", "path": "two.rrd"},
    ]

    assert _resolve_huggingface_rrd_urls_from_tree("owner/data", "branch/name", tree, limit=1) == [
        "https://huggingface.co/datasets/owner/data/resolve/branch%2Fname/one.rrd"
    ]


def test_huggingface_next_link_parses_next_relation() -> None:
    header = (
        '<https://huggingface.co/api/datasets/rerun/droid_sample/tree/main?cursor=abc>; rel="next", '
        '<https://huggingface.co/api/datasets/rerun/droid_sample/tree/main?cursor=last>; rel="last"'
    )

    assert (
        _huggingface_next_link(header)
        == "https://huggingface.co/api/datasets/rerun/droid_sample/tree/main?cursor=abc"
    )
    assert _huggingface_next_link(None) is None
