from __future__ import annotations

import hashlib
import json
from argparse import ArgumentParser
from pathlib import Path
from typing import Any
from urllib.parse import urlencode

import requests
import rerun as rr
import rerun.blueprint as rrb

CACHE_DIR = Path(__file__).parent / "cache"
if not CACHE_DIR.exists():
    CACHE_DIR.mkdir()

OVERPASS_API_URL = "https://overpass-api.de/api/interpreter"

# Find some hotels in the area of Lausanne, Switzerland
# This uses the Overpass API query language: https://wiki.openstreetmap.org/wiki/Overpass_API/Overpass_QL
HOTELS_IN_LAUSANNE_QUERY = """
[out:json][timeout:90];
(
    nw["tourism"="hotel"]
    (46.4804134576154,6.533088904998318,46.647800100605984,6.7706675099821165);
);
out geom;
"""


def execute_query(query: str) -> dict[str, Any]:
    """Execute an Overpass query, caching its result locally."""
    query_hash = hashlib.sha256(query.encode()).hexdigest()

    cache_file = CACHE_DIR / f"{query_hash}.json"
    if cache_file.exists():
        result = json.loads(cache_file.read_text())
    else:
        params = {"data": query}
        encoded_query = urlencode(params)
        headers = {"Content-Type": "application/x-www-form-urlencoded"}
        response = requests.post(OVERPASS_API_URL, data=encoded_query, headers=headers)

        if not response.ok:
            raise RuntimeError(f"Overpass API request failed: {response.status_code} {response.text}")

        result = response.json()

        cache_file.write_text(json.dumps(result))

    # very basic validation
    if not isinstance(result, dict):
        raise ValueError("Unexpected result from Overpass API")

    return result


def log_node(node: dict[str, Any]) -> None:
    node_id = node["id"]
    entity_path = f"nodes/{node_id}"

    rr.log(
        entity_path,
        rr.GeoPoints(lat_lon=[node["lat"], node["lon"]], radii=rr.components.Radius.ui_points(7.0)),
        rr.AnyValues(**node.get("tags", {})),
        static=True,
    )


def log_way(way: dict[str, Any]) -> None:
    way_id = way["id"]
    entity_path = f"ways/{way_id}"

    coords = [(node["lat"], node["lon"]) for node in way["geometry"]]

    rr.log(
        entity_path,
        rr.GeoLineStrings(lat_lon=[coords], radii=rr.components.Radius.ui_points(2.0)),
        rr.AnyValues(**way.get("tags", {})),
        static=True,
    )


def log_data(data: dict[str, Any]) -> None:
    try:
        copyright_text = data["osm3s"]["copyright"]
        rr.log("copyright", rr.TextDocument(copyright_text), static=True)
    except KeyError:
        pass

    for element in data["elements"]:
        if element["type"] == "node":
            log_node(element)
        elif element["type"] == "way":
            log_way(element)


def main() -> None:
    parser = ArgumentParser(description="Visualize OpenStreetMap data")
    rr.script_add_args(parser)
    args = parser.parse_args()

    blueprint = rrb.Blueprint(
        rrb.MapView(origin="/"),
        collapse_panels=True,
    )

    rr.script_setup(args, "rerun_example_openstreetmap_data", default_blueprint=blueprint)

    data = execute_query(HOTELS_IN_LAUSANNE_QUERY)
    log_data(data)


if __name__ == "__main__":
    main()
