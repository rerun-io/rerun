from __future__ import annotations

import dataclasses
import io
import itertools
import json
import re
import zipfile
from argparse import ArgumentParser
from pathlib import Path
from typing import Any

import geopandas as gpd
import numpy as np
import numpy.typing as npt
import requests
import rerun as rr
import shapely
from pyproj import CRS, Transformer
from pyproj.aoi import AreaOfInterest
from pyproj.database import query_utm_crs_info
from tqdm import tqdm

DATA_DIR = Path(__file__).parent / "data"
MAP_DATA_DIR = DATA_DIR / "map_data"
if not DATA_DIR.exists():
    DATA_DIR.mkdir()

INVOLI_DATASETS: dict[str, str] = {}  # TODO(ab): add some datasets


def download_with_progress(url: str, what: str) -> io.BytesIO:
    """Download file with tqdm progress bar."""
    chunk_size = 1024 * 1024
    resp = requests.get(url, stream=True)
    total_size = int(resp.headers.get("content-length", 0))
    with tqdm(desc=f"Downloading {what}…", total=total_size, unit="iB", unit_scale=True, unit_divisor=1024) as progress:
        download_file = io.BytesIO()
        for data in resp.iter_content(chunk_size):
            download_file.write(data)
            progress.update(len(data))

    download_file.seek(0)
    return download_file


def shapely_geom_to_numpy(geom: shapely.Geometry) -> list[npt.NDArray[np.float64]]:
    """Convert shapely objects to numpy array suitable for logging as line batches."""

    if geom.geom_type == "Polygon":
        return [np.array(geom.exterior.coords)] + [np.array(interior.coords) for interior in geom.interiors]
    elif geom.geom_type == "MultiPolygon":
        res = []
        for poly in geom.geoms:
            res.extend(shapely_geom_to_numpy(poly))
        return res
    else:
        print(f"Warning: unknown Shapely object {geom}")
        return []


def log_region_boundaries_for_country(
    country_code: str, level: int, color: tuple[float, float, float], crs: CRS
) -> None:
    """Log some boundaries for the given country and level."""

    def download_eu_map_data() -> None:
        """Download some basic EU map data."""

        if MAP_DATA_DIR.exists():
            return

        EU_MAP_DATA_URL = "https://gisco-services.ec.europa.eu/distribution/v2/nuts/download/ref-nuts-2021-01m.json.zip"
        zip_data = download_with_progress(EU_MAP_DATA_URL, "map data")
        with zipfile.ZipFile(zip_data) as zip_ref:
            zip_ref.extractall(MAP_DATA_DIR)

    download_eu_map_data()

    # cspell:disable-next-line
    map_data = gpd.read_file(MAP_DATA_DIR / f"NUTS_RG_01M_2021_4326_LEVL_{level}.json").set_crs("epsg:4326").to_crs(crs)

    for i, row in map_data[map_data.CNTR_CODE == country_code].iterrows():
        entity_path = f"region_boundaries/{country_code}/{level}/{row.NUTS_ID}"
        lines = shapely_geom_to_numpy(row.geometry)
        rr.log(entity_path + "/2D", rr.LineStrips2D(lines, colors=color), timeless=True)
        rr.log(
            entity_path + "/3D",
            rr.LineStrips3D([np.hstack([line, np.zeros((len(line), 1))]) for line in lines], colors=color),
            timeless=True,
        )
        metadata = row.to_dict()
        metadata.pop("geometry")
        rr.log(entity_path, rr.AnyValues(**metadata), timeless=True)


@dataclasses.dataclass
class Measurement:
    """One measurement loaded from INVOLI data. Corresponds to an "aircraft" record."""

    icao_id: str
    latitude: float | None
    longitude: float | None
    barometric_altitude: float | None
    wg84_altitude: float | None
    course: float | None
    ground_speed: float | None
    vertical_speed: float | None
    ground_status: str | None
    timestamp: float

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> Measurement:
        return cls(
            icao_id=data["ids"]["icao"],
            latitude=data.get("latitude"),
            longitude=data.get("longitude"),
            barometric_altitude=data.get("barometric_altitude"),
            wg84_altitude=data.get("wg84_altitude"),
            course=data.get("course"),
            ground_speed=data.get("ground_speed"),
            vertical_speed=data.get("vertical_speed"),
            ground_status=data.get("ground_status"),
            timestamp=data["timestamp"][0] + data["timestamp"][1] / 1e9,
        )


def find_best_utm_crs(measurements: list[Measurement]) -> CRS:
    """Returns the best UTM coordinates reference system given a list of measurements."""

    def get_area_of_interest(measurements: list[Measurement]) -> AreaOfInterest:
        """Compute the span of coordinates for all provided measurements."""

        print("Computing area of interest...")
        all_long_lat = [
            (a.longitude, a.latitude) for a in measurements if a.latitude is not None and a.longitude is not None
        ]
        return AreaOfInterest(
            west_lon_degree=min(x[0] for x in all_long_lat),
            south_lat_degree=min(x[1] for x in all_long_lat),
            east_lon_degree=max(x[0] for x in all_long_lat),
            north_lat_degree=max(x[1] for x in all_long_lat),
        )

    area_of_interest = get_area_of_interest(measurements)
    utm_crs_list = query_utm_crs_info(
        datum_name="WGS 84",
        area_of_interest=area_of_interest,
    )

    return CRS.from_epsg(utm_crs_list[0].code)


def load_measurements(paths: list[Path]) -> list[Measurement]:
    """Load measurements from a bunch of json files."""
    all_measurements = []
    for path in tqdm(paths, "Loading measurements"):
        data = json.loads(path.read_text())
        for data_rec in data:
            for aircraft in data_rec["records"]:
                all_measurements.append(Measurement.from_dict(aircraft["aircraft"]))

    return all_measurements


def get_paths_for_directory(directory: Path) -> list[Path]:
    """
    Get a sorted list of JSON file by recursively walking the provided directory.

    Note: technically, we don't need the list to be sorted as Rerun accepts out of order data. However, it comes at a
    (small) performance cost and any (cheap) sorting on the logging end is always better.
    """

    def atoi(text: str) -> int | str:
        return int(text) if text.isdigit() else text

    def natural_keys(path: Path) -> list[int | str]:
        """
        Human sort.

        alist.sort(key=natural_keys) sorts in human order
        https://nedbatchelder.com/blog/200712/human_sorting.html
        (See Toothy's implementation in the comments)
        """
        return [atoi(c) for c in re.split(r"(\d+)", str(path))]

    return sorted(directory.rglob("*.json"), key=natural_keys)


def log_everything(paths: list[Path]) -> None:
    measurements = load_measurements(paths)
    utm_crs = find_best_utm_crs(measurements)

    proj = Transformer.from_crs("EPSG:4326", utm_crs, always_xy=True)

    rr.set_time_seconds("unix_time", 0)
    for country_code, (level, color) in itertools.product(["DE", "FR", "CH"], [(0, (1, 0.5, 0.5))]):
        log_region_boundaries_for_country(country_code, level, color, utm_crs)

    # Exaggerate altitudes
    rr.log("aircraft", rr.Transform3D(rr.TranslationRotationScale3D(scale=[1, 1, 10])), timeless=True)

    for measurement in tqdm(measurements, "Logging measurements"):
        rr.set_time_seconds("unix_time", measurement.timestamp)

        metadata = rr.AnyValues(**dataclasses.asdict(measurement))
        entity_path = f"aircraft/{measurement.icao_id}"

        if (
            measurement.latitude is not None
            and measurement.longitude is not None
            and measurement.barometric_altitude is not None
        ):
            rr.log(
                entity_path,
                rr.Points3D(
                    [proj.transform(measurement.longitude, measurement.latitude, measurement.barometric_altitude)]
                ),
                metadata,
            )
        else:
            rr.log(entity_path, metadata)

        if measurement.barometric_altitude is not None:
            rr.log(
                entity_path + "/barometric_altitude",
                rr.Scalar(measurement.barometric_altitude),
            )


def main() -> None:
    parser = ArgumentParser(description="Visualize INVOLI data")
    parser.add_argument(
        "--dataset",
        choices=INVOLI_DATASETS.keys(),
        default="10min",
        help="Which dataset to automatically download and visualize",
    )
    parser.add_argument("--dir", type=Path, help="Use this directory of data instead of downloading a dataset")
    rr.script_add_args(parser)
    args = parser.parse_args()

    if args.dir:
        dataset_directory = args.dir
    else:
        dataset = args.dataset
        dataset_ulr = INVOLI_DATASETS[dataset]
        dataset_directory = DATA_DIR / dataset
        if not dataset_directory.exists():
            dataset_directory.mkdir()
            zip_data = download_with_progress(dataset_ulr, f"dataset {dataset}")
            with zipfile.ZipFile(zip_data) as zip_ref:
                zip_ref.extractall(dataset_directory)

    rr.script_setup(args, "rerun_example_air_traffic_data")

    paths = get_paths_for_directory(dataset_directory)
    log_everything(paths)


if __name__ == "__main__":
    main()
