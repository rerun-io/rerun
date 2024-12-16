from __future__ import annotations

import dataclasses
import io
import itertools
import json
import re
import typing
import zipfile
from argparse import ArgumentParser
from pathlib import Path
from typing import Any

import geopandas as gpd
import numpy as np
import numpy.typing as npt
import polars
import pyproj
import requests
import rerun as rr
import rerun.blueprint as rrb
import shapely
from pyproj import CRS, Transformer
from pyproj.aoi import AreaOfInterest
from pyproj.database import query_utm_crs_info
from tqdm import tqdm

DATA_DIR = Path(__file__).parent / "dataset"
MAP_DATA_DIR = DATA_DIR / "map_data"
if not DATA_DIR.exists():
    DATA_DIR.mkdir()

INVOLI_DATASETS = {
    "10min": "https://storage.googleapis.com/rerun-example-datasets/involi/involi_demo_set_1_10min.zip",
    "2h": "https://storage.googleapis.com/rerun-example-datasets/involi/involi_demo_set_2_2h.zip",
}


def download_with_progress(url: str, what: str) -> io.BytesIO:
    """Download file with tqdm progress bar."""
    chunk_size = 1024 * 1024
    resp = requests.get(url, stream=True)
    total_size = int(resp.headers.get("content-length", 0))
    with tqdm(
        desc=f"Downloading {what}…",
        total=total_size,
        unit="iB",
        unit_scale=True,
        unit_divisor=1024,
    ) as progress:
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
        rr.log(entity_path + "/2D", rr.LineStrips2D(lines, colors=color), static=True)
        rr.log(
            entity_path + "/3D",
            rr.LineStrips3D(
                [np.hstack([line, np.zeros((len(line), 1))]) for line in lines],
                colors=color,
            ),
            static=True,
        )
        metadata = row.to_dict()
        metadata.pop("geometry")
        rr.log(entity_path, rr.AnyValues(**metadata), static=True)


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


class Logger(typing.Protocol):
    def process_measurement(self, measurement: Measurement) -> None:
        pass

    def flush(self) -> None:
        pass


# ================================================================================================
# Simple logger


class MeasurementLogger:
    """Logger class that uses regular `rr.log` calls."""

    def __init__(self, proj: pyproj.Transformer, raw: bool):
        self._proj = proj
        self._raw = raw

        self._ignored_fields = [
            "icao_id",  # already the entity's path
            "timestamp",  # already the clock's value
        ]

    def process_measurement(self, measurement: Measurement) -> None:
        rr.set_time_seconds("unix_time", measurement.timestamp)

        if self._raw:
            metadata = dataclasses.asdict(measurement)
        else:
            metadata = dataclasses.asdict(
                measurement,
                dict_factory=lambda x: {k: v for (k, v) in x if k not in self._ignored_fields and v is not None},
            )

        entity_path = f"aircraft/{measurement.icao_id}"
        color = rr.components.Color.from_string(entity_path)

        if (
            measurement.latitude is not None
            and measurement.longitude is not None
            and measurement.barometric_altitude is not None
        ):
            rr.log(
                entity_path,
                rr.Points3D(
                    [
                        self._proj.transform(
                            measurement.longitude,
                            measurement.latitude,
                            measurement.barometric_altitude,
                        ),
                    ],
                    colors=color,
                ),
                rr.GeoPoints(lat_lon=[measurement.latitude, measurement.longitude]),
            )

        if len(metadata) > 0:
            rr.log(entity_path, rr.AnyValues(**metadata))

        if measurement.barometric_altitude is not None:
            rr.log(
                entity_path + "/barometric_altitude",
                rr.Scalar(measurement.barometric_altitude),
                rr.SeriesLine(color=color),
            )

    def flush(self) -> None:
        pass


# ================================================================================================
# Batch logger


class MeasurementBatchLogger:
    """Logger class that batches measurements and uses `rr.send_columns` calls."""

    def __init__(self, proj: pyproj.Transformer, batch_size: int = 8192):
        self._proj = proj
        self._batch_size = batch_size
        self._measurements: list[Measurement] = []
        self._position_indicators: set[str] = set()

    def process_measurement(self, measurement: Measurement) -> None:
        self._measurements.append(measurement)

        if len(self._measurements) >= 8192:
            self.flush()

    def flush(self) -> None:
        # !!! the raw data is not sorted by timestamp, so we sort it here
        df = polars.DataFrame(self._measurements).sort("timestamp")
        self._measurements = []

        for (icao_id,), group in df.group_by("icao_id"):
            icao_id = str(icao_id)

            # Note: this splitting in 3 different functions is due to the pattern of nulls in the raw data.
            self.log_position_and_altitude(group, icao_id)
            self.log_ground_status(group, icao_id)
            self.log_metadata(group, icao_id)

    def log_position_and_altitude(self, df: polars.DataFrame, icao_id: str) -> None:
        entity_path = f"aircraft/{icao_id}"
        df = df["timestamp", "latitude", "longitude", "barometric_altitude"].drop_nulls()

        if df.height == 0:
            return

        if icao_id not in self._position_indicators:
            color = rr.components.Color.from_string(entity_path)
            rr.log(
                entity_path,
                [rr.archetypes.Points3D.indicator(), rr.archetypes.GeoPoints.indicator(), color],
                static=True,
            )
            rr.log(entity_path + "/barometric_altitude", [rr.archetypes.SeriesLine.indicator(), color], static=True)
            self._position_indicators.add(icao_id)

        timestamps = rr.TimeSecondsColumn("unix_time", df["timestamp"].to_numpy())
        pos = self._proj.transform(df["longitude"], df["latitude"], df["barometric_altitude"])
        positions = rr.components.Position3DBatch(np.vstack(pos).T)

        lat_lon = rr.components.LatLonBatch(np.vstack((df["latitude"], df["longitude"])).T)

        raw_coordinates = rr.AnyValues(
            latitude=df["latitude"].to_numpy(),
            longitude=df["longitude"].to_numpy(),
            barometric_altitude=df["barometric_altitude"].to_numpy(),
        )

        rr.send_columns(
            entity_path,
            [timestamps],
            [
                positions,
                lat_lon,
                *raw_coordinates.as_component_batches(),
            ],
        )

        rr.send_columns(
            entity_path + "/barometric_altitude",
            [timestamps],
            [rr.components.ScalarBatch(df["barometric_altitude"].to_numpy())],
        )

    def log_ground_status(self, df: polars.DataFrame, icao_id: str) -> None:
        entity_path = f"aircraft/{icao_id}"
        df = df["timestamp", "ground_status"].drop_nulls()

        timestamps = rr.TimeSecondsColumn("unix_time", df["timestamp"].to_numpy())
        batches = rr.AnyValues(ground_status=df["ground_status"].to_numpy())

        rr.send_columns(entity_path, [timestamps], batches.as_component_batches())

    def log_metadata(self, df: polars.DataFrame, icao_id: str) -> None:
        entity_path = f"aircraft/{icao_id}"
        df = df["timestamp", "course", "ground_speed", "vertical_speed"].drop_nulls()

        metadata = rr.AnyValues(
            course=df["course"].to_numpy(),
            ground_speed=df["ground_speed"].to_numpy(),
            vertical_speed=df["vertical_speed"].to_numpy(),
        )

        rr.send_columns(
            entity_path,
            [rr.TimeSecondsColumn("unix_time", df["timestamp"].to_numpy())],
            metadata.component_batches,
        )


# ================================================================================================


def log_everything(paths: list[Path], raw: bool, batch: bool, batch_size: int) -> None:
    measurements = load_measurements(paths)
    utm_crs = find_best_utm_crs(measurements)

    proj = Transformer.from_crs("EPSG:4326", utm_crs, always_xy=True)

    rr.set_time_seconds("unix_time", 0)
    for country_code, (level, color) in itertools.product(["DE", "CH"], [(0, (1, 0.5, 0.5))]):
        log_region_boundaries_for_country(country_code, level, color, utm_crs)

    # Exaggerate altitudes
    rr.log("aircraft", rr.Transform3D(scale=[1, 1, 10]), static=True)

    if batch:
        logger: Logger = MeasurementBatchLogger(proj, batch_size)
    else:
        logger = MeasurementLogger(proj, raw)

    for measurement in tqdm(measurements, "Logging measurements"):
        if measurement.icao_id is None:
            continue

        logger.process_measurement(measurement)
    logger.flush()


def main() -> None:
    parser = ArgumentParser(description="Visualize INVOLI data")
    parser.add_argument(
        "--dataset",
        choices=INVOLI_DATASETS.keys(),
        default="2h",
        help="Which dataset to automatically download and visualize",
    )
    parser.add_argument(
        "--raw",
        action="store_true",
        help="If true, logs the raw data with all its issues (useful to stress edge cases in the viewer)",
    )
    parser.add_argument(
        "--batch",
        action="store_true",
        default=True,
        help="If true, use the batch logger function (rerun 0.18 required)",
    )
    parser.add_argument(
        "--batch-size",
        type=int,
        default=8192,
        help="Batch size for the batch logger",
    )
    parser.add_argument(
        "--dir",
        type=Path,
        help="Use this directory of data instead of downloading a dataset",
    )
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

    # TODO(ab): this blueprint would be massively improved by setting the 3D view's orbit point to FRA's coordinates.
    blueprint = rrb.Vertical(
        rrb.Horizontal(rrb.Spatial3DView(origin="/"), rrb.MapView(origin="/")),
        rrb.TimeSeriesView(origin="/aircraft"),
        row_shares=[3, 1],
    )
    rr.script_setup(args, "rerun_example_air_traffic_data", default_blueprint=blueprint)

    paths = get_paths_for_directory(dataset_directory)
    log_everything(paths, args.raw, args.batch, args.batch_size)


if __name__ == "__main__":
    main()
