from __future__ import annotations

import time

import rerun as rr

CATALOG_URL = "rerun+http://localhost:51234"


client = rr.catalog.CatalogClient(CATALOG_URL)
# dfn.html_formatter.set_formatter(RerunHtmlTable(None, None))
all_entries = client.all_entries()
first_entry = all_entries[0]

dataset = client.get_dataset_entry(name=first_entry.name)

top_view = dataset.dataframe_query_view(index=None, contents="/observation.images.Lwebcam/**").df()
first_item = top_view.limit(1)
collected_item = first_item.collect()
print("Can collect")
html = first_item._repr_html_()
print("Can get html")

start = time.time()
regular_repr = first_item.__repr__()
print(f"Took {time.time() - start}s for full repr")

# rr.AssetVideo
