#!/usr/bin/env python3
"""Draws assets/img/world_map.png, the land mask behind SERVER INFO's server dots.

Land outlines: Natural Earth 1:110m land (public domain), in the Miller projection between
MIN_LAT and MAX_LAT, as white with the land coverage in alpha (the launcher tints it).
Keep MIN_LAT/MAX_LAT in sync with src/ui/launcher/server_info.rs.

  python3 scripts/world_map.py
"""

import json
import math
import struct
import urllib.request
import zlib
from pathlib import Path

SOURCE = "https://raw.githubusercontent.com/nvkelso/natural-earth-vector/master/geojson/ne_110m_land.geojson"
OUT = Path(__file__).resolve().parent.parent / "assets" / "img" / "world_map.png"
MIN_LAT, MAX_LAT = -73.5, 83.6
W = 1000
SS = 3  # supersampling per axis


def miller(lat: float) -> float:
    return 1.25 * math.log(math.tan(math.pi / 4 + 0.4 * math.radians(lat)))


Y_TOP, Y_BOTTOM = miller(MAX_LAT), miller(MIN_LAT)
H = round(W * (Y_TOP - Y_BOTTOM) / (2 * math.pi))


def project(lon: float, lat: float, w: int, h: int) -> tuple[float, float]:
    lat = max(min(lat, MAX_LAT), MIN_LAT)
    return (lon + 180) / 360 * w, (Y_TOP - miller(lat)) / (Y_TOP - Y_BOTTOM) * h


def rings(geojson: dict) -> list[list[tuple[float, float]]]:
    out = []
    for f in geojson["features"]:
        g = f["geometry"]
        polys = [g["coordinates"]] if g["type"] == "Polygon" else g["coordinates"]
        for poly in polys:
            out.extend(poly)
    return out


def rasterize(all_rings, w: int, h: int) -> bytearray:
    """Even-odd scanline fill at pixel centres; 1 = land."""
    mask = bytearray(w * h)
    for ring in all_rings:
        pts = [project(lon, lat, w, h) for lon, lat in ring]
        edges = [(pts[i], pts[(i + 1) % len(pts)]) for i in range(len(pts))]
        ys = [p[1] for p in pts]
        for y in range(max(0, int(min(ys))), min(h, int(max(ys)) + 1)):
            cy = y + 0.5
            xs = sorted(
                a[0] + (cy - a[1]) * (b[0] - a[0]) / (b[1] - a[1])
                for a, b in edges
                if (a[1] <= cy) != (b[1] <= cy)
            )
            row = y * w
            for x0, x1 in zip(xs[::2], xs[1::2]):
                for x in range(max(0, math.ceil(x0 - 0.5)), min(w, math.ceil(x1 - 0.5))):
                    mask[row + x] ^= 1
    return mask


def png(path: Path, w: int, h: int, alpha: bytes) -> None:
    raw = b"".join(b"\x00" + b"".join(b"\xff\xff\xff" + bytes([a]) for a in alpha[y * w:(y + 1) * w]) for y in range(h))

    def chunk(kind: bytes, data: bytes) -> bytes:
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))

    path.write_bytes(
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 6, 0, 0, 0))
        + chunk(b"IDAT", zlib.compress(raw, 9))
        + chunk(b"IEND", b"")
    )


def main() -> None:
    with urllib.request.urlopen(SOURCE, timeout=60) as r:
        land = rings(json.load(r))
    big = rasterize(land, W * SS, H * SS)
    alpha = bytearray(W * H)
    for y in range(H):
        for x in range(W):
            n = sum(big[(y * SS + dy) * W * SS + x * SS + dx] for dy in range(SS) for dx in range(SS))
            alpha[y * W + x] = round(255 * n / (SS * SS))
    png(OUT, W, H, bytes(alpha))
    print(f"wrote {OUT} ({W}x{H})")


if __name__ == "__main__":
    main()
