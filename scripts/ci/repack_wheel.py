#!/usr/bin/env python3
"""
Add files to a wheel without recompressing its existing entries.

Existing entries are copied as raw compressed bytes, added files are deflated in parallel,
and `RECORD` is rewritten to cover the added files.

Usage:
    python3 scripts/ci/repack_wheel.py <wheel> <output_dir> --add <src> <dest> [--add <src> <dest> …]

`<src>` may be a file or a directory. Directories are added recursively under `<dest>`.
Added files replace existing entries with the same path.
"""

from __future__ import annotations

import argparse
import base64
import csv
import hashlib
import io
import os
import posixpath
import re
import stat
import struct
import sys
import time
import zipfile
import zlib
from concurrent.futures import ThreadPoolExecutor
from dataclasses import dataclass
from pathlib import Path

COMPRESS_LEVEL = zlib.Z_DEFAULT_COMPRESSION
CHUNK_SIZE = 4 * 1024 * 1024
DEFLATE_WINDOW = 1 << zlib.MAX_WBITS
MIN_ZIP_TIMESTAMP = 315532800
ZIP32_LIMIT = 0xFFFFFFFF
RECORD_RE = re.compile(r"^[^/]+\.dist-info/RECORD$")


@dataclass
class Entry:
    name: str
    compress_type: int
    date_time: tuple[int, int, int, int, int, int]
    create_system: int
    create_version: int
    external_attr: int
    crc: int
    file_size: int
    data: bytes


def deflate_chunk(data: memoryview, start: int, end: int) -> bytes:
    if start == 0:
        compressor = zlib.compressobj(COMPRESS_LEVEL, zlib.DEFLATED, -zlib.MAX_WBITS)
    else:
        zdict = bytes(data[max(0, start - DEFLATE_WINDOW) : start])
        compressor = zlib.compressobj(COMPRESS_LEVEL, zlib.DEFLATED, -zlib.MAX_WBITS, zdict=zdict)
    out = compressor.compress(data[start:end])
    out += compressor.flush(zlib.Z_FINISH if end == len(data) else zlib.Z_SYNC_FLUSH)
    return out


def record_hash(data: bytes) -> str:
    digest = hashlib.sha256(data).digest()
    return "sha256=" + base64.urlsafe_b64encode(digest).rstrip(b"=").decode("ascii")


def zip_date_time(mtime: float) -> tuple[int, int, int, int, int, int]:
    t = time.gmtime(max(mtime, MIN_ZIP_TIMESTAMP))
    return (t.tm_year, t.tm_mon, t.tm_mday, t.tm_hour, t.tm_min, t.tm_sec)


def collect_added_files(adds: list[tuple[str, str]]) -> dict[str, Path]:
    files: dict[str, Path] = {}
    for src, dest in adds:
        src_path = Path(src)
        dest = posixpath.normpath(dest).strip("/")
        if dest == ".":
            dest = ""
        if src_path.is_file():
            files[dest] = src_path
        elif src_path.is_dir():
            for root, dirnames, filenames in os.walk(src_path):
                dirnames.sort()
                for filename in sorted(filenames):
                    path = Path(root, filename)
                    if path.is_file():
                        rel = path.relative_to(src_path).as_posix()
                        files[f"{dest}/{rel}" if dest else rel] = path
        else:
            raise FileNotFoundError(f"Not found: {src_path}")
    return files


def read_raw_entries(wheel: Path) -> tuple[list[Entry], str, list[list[str]]]:
    entries: list[Entry] = []
    record_name = None
    record_rows: list[list[str]] = []
    with zipfile.ZipFile(wheel) as zf, open(wheel, "rb") as f:
        for info in zf.infolist():
            if RECORD_RE.match(info.filename):
                if record_name is not None:
                    raise ValueError(f"Multiple RECORD files in {wheel}")
                record_name = info.filename
                record_rows = list(csv.reader(io.StringIO(zf.read(info).decode("utf-8"))))
                continue
            if info.flag_bits & 0x1:
                raise ValueError(f"Encrypted entry in {wheel}: {info.filename}")
            if info.compress_type not in (zipfile.ZIP_STORED, zipfile.ZIP_DEFLATED):
                raise ValueError(f"Unsupported compression for {info.filename}: {info.compress_type}")
            f.seek(info.header_offset)
            header = f.read(30)
            if header[:4] != b"PK\x03\x04":
                raise ValueError(f"Bad local header for {info.filename}")
            name_len, extra_len = struct.unpack("<HH", header[26:30])
            f.seek(info.header_offset + 30 + name_len + extra_len)
            entries.append(
                Entry(
                    name=info.filename,
                    compress_type=info.compress_type,
                    date_time=info.date_time,
                    create_system=info.create_system,
                    create_version=info.create_version,
                    external_attr=info.external_attr,
                    crc=info.CRC,
                    file_size=info.file_size,
                    data=f.read(info.compress_size),
                )
            )
    if record_name is None:
        raise ValueError(f"No RECORD file in {wheel}")
    return entries, record_name, record_rows


def compress_files(files: dict[str, Path]) -> tuple[list[Entry], dict[str, tuple[str, int]]]:
    contents = {name: path.read_bytes() for name, path in files.items()}
    with ThreadPoolExecutor(max_workers=os.cpu_count()) as pool:
        chunk_futures = {
            name: [
                pool.submit(deflate_chunk, memoryview(data), start, min(start + CHUNK_SIZE, len(data)))
                for start in range(0, max(len(data), 1), CHUNK_SIZE)
            ]
            for name, data in contents.items()
        }
        hash_futures = {name: pool.submit(record_hash, data) for name, data in contents.items()}

        entries = []
        hashes = {}
        for name, path in files.items():
            data = contents[name]
            st = path.stat()
            entries.append(
                Entry(
                    name=name,
                    compress_type=zipfile.ZIP_DEFLATED,
                    date_time=zip_date_time(st.st_mtime),
                    create_system=3,
                    create_version=20,
                    external_attr=(stat.S_IMODE(st.st_mode) | stat.S_IFMT(st.st_mode)) << 16,
                    crc=zlib.crc32(data),
                    file_size=len(data),
                    data=b"".join(future.result() for future in chunk_futures[name]),
                )
            )
            hashes[name] = (hash_futures[name].result(), len(data))
    return entries, hashes


def write_zip(path: Path, entries: list[Entry]) -> None:
    if len(entries) >= 0xFFFF:
        raise ValueError("Too many entries for a zip without zip64")
    central = io.BytesIO()
    with open(path, "wb") as f:
        for entry in entries:
            offset = f.tell()
            if max(offset, entry.file_size, len(entry.data)) > ZIP32_LIMIT:
                raise ValueError(f"{entry.name} needs zip64, which is not supported")
            name = entry.name.encode("utf-8")
            year, month, day, hour, minute, second = entry.date_time
            dos_time = (hour << 11) | (minute << 5) | (second // 2)
            dos_date = ((year - 1980) << 9) | (month << 5) | day
            common = struct.pack(
                "<HHHHHIIIH",
                20,
                0x0800,
                entry.compress_type,
                dos_time,
                dos_date,
                entry.crc,
                len(entry.data),
                entry.file_size,
                len(name),
            )
            f.write(b"PK\x03\x04" + common + struct.pack("<H", 0) + name)
            f.write(entry.data)
            central.write(
                b"PK\x01\x02"
                + struct.pack("<H", (entry.create_system << 8) | entry.create_version)
                + common
                + struct.pack("<HHHHII", 0, 0, 0, 0, entry.external_attr, offset)
                + name
            )
        central_offset = f.tell()
        central_data = central.getvalue()
        if central_offset + len(central_data) > ZIP32_LIMIT:
            raise ValueError("Wheel needs zip64, which is not supported")
        f.write(central_data)
        f.write(
            b"PK\x05\x06"
            + struct.pack("<HHHHIIH", 0, 0, len(entries), len(entries), len(central_data), central_offset, 0)
        )


def repack(wheel: Path, output_dir: Path, adds: list[tuple[str, str]]) -> Path:
    added_files = collect_added_files(adds)
    old_entries, record_name, record_rows = read_raw_entries(wheel)
    new_entries, new_hashes = compress_files(added_files)

    kept_entries = [entry for entry in old_entries if entry.name not in added_files]
    dist_info = record_name.split("/")[0] + "/"
    ordered = (
        [entry for entry in kept_entries if not entry.name.startswith(dist_info)]
        + new_entries
        + [entry for entry in kept_entries if entry.name.startswith(dist_info)]
    )

    record = io.StringIO()
    writer = csv.writer(record, delimiter=",", quotechar='"', lineterminator="\n")
    writer.writerows(row for row in record_rows if row and row[0] != record_name and row[0] not in added_files)
    writer.writerows((name, hash_, size) for name, (hash_, size) in new_hashes.items())
    writer.writerow((record_name, "", ""))
    record_data = record.getvalue().encode("utf-8")
    record_compressor = zlib.compressobj(COMPRESS_LEVEL, zlib.DEFLATED, -zlib.MAX_WBITS)
    ordered.append(
        Entry(
            name=record_name,
            compress_type=zipfile.ZIP_DEFLATED,
            date_time=zip_date_time(time.time()),
            create_system=3,
            create_version=20,
            external_attr=(0o664 | stat.S_IFREG) << 16,
            crc=zlib.crc32(record_data),
            file_size=len(record_data),
            data=record_compressor.compress(record_data) + record_compressor.flush(),
        )
    )

    output_dir.mkdir(parents=True, exist_ok=True)
    output = output_dir / wheel.name
    write_zip(output, ordered)

    with zipfile.ZipFile(output) as zf:
        bad = zf.testzip()
        if bad is not None:
            raise ValueError(f"CRC mismatch for {bad} in {output}")
    return output


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Add files to a wheel without recompressing its existing entries.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=__doc__,
    )
    parser.add_argument("wheel", type=Path, help="Wheel to add files to")
    parser.add_argument("output_dir", type=Path, help="Directory to write the repacked wheel to")
    parser.add_argument(
        "--add",
        nargs=2,
        action="append",
        default=[],
        metavar=("SRC", "DEST"),
        help="File or directory to add, and its path inside the wheel",
    )
    args = parser.parse_args()

    start = time.perf_counter()
    output = repack(args.wheel, args.output_dir, args.add)
    print(f"Wrote {output} ({output.stat().st_size} bytes) in {time.perf_counter() - start:.1f}s")
    return 0


if __name__ == "__main__":
    sys.exit(main())
